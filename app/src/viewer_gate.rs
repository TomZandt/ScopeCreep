//! Which Feed frames are **Viewer Frames**: the pacing gate.
//!
//! Resolve flags thumbnail renders (the Sender never feeds those) but not background cache renders or repeat
//! renders, and a plugin cannot see the playhead (Stage 0, ticket #11). So the Sender feeds every other render
//! and this gate decides from pacing alone, on the Sender's clock (`render_ns`). It is the doorman watching people
//! arrive: a steady queue is playback, one person turning up is a scrub or a step, and a coach party pouring
//! through the door in numbered order, faster than any queue can move, is a cache fill.
//!
//! Frames are grouped into **runs**: frames whose numbers follow on (one to three apart, in one direction) from
//! the same Sender instance with no gap longer than a hitch. What gives a cache fill away is its average pace:
//! it renders as fast as the GPU allows (in Resolve 21.2 at HD, 5 to 12 ms a frame, often two frames at once
//! from two threads), while playback never runs faster than the timeline rate and a user stepping with a held
//! arrow key steps no faster than key repeat (about 30 ms). A run whose average interval over its last
//! [`WINDOW`] renders (at least [`MIN_CACHE_FILL_RUN`]) is below both 0.83 of a period and [`FILL_INTERVAL_NS`] is
//! being a cache fill, provided the run is also **ahead of its own clock**: its newest frame came more than a
//! period earlier than any frame before it in the run said it was due. Playback can run late and then catch up
//! (frames bunched after a stall), but never ahead. The judgement is about the recent frames, not the whole run, and it can be withdrawn: a
//! cold playback start in Resolve 21.2 renders its first second out of order and in bursts that look like a fill,
//! then settles; once the last [`WINDOW`] frames came at the frame rate (at least 0.9 of a period each on
//! average) the run is playback again and shown.
//!
//! - **Repeat render** (same Sender instance, same frame number as the previous render): the picture is updated,
//!   because a grade may have changed, but it is not a new Viewer Frame.
//! - **A new run is verified first**: its newest frame is held back (nothing is shown) until the run has lasted
//!   [`verify_ns`] without being a cache fill, its recent frames arriving one at a time (then it is playback, and from then on every frame is shown at once),
//!   or it ends (then it was the user, a step or a scrub, and its newest frame is shown), or it turns out to be a
//!   cache fill (then none of it is shown and the Picture View keeps the last Viewer Frame).
//! - **A drag is followed** (ADR 0031, #104): a run that goes past [`MIN_CACHE_FILL_RUN`] renders without being
//!   judged a fill is a user dragging the playhead (or playback), and from then on every frame of it is shown at
//!   once, the stop frame included. A fill is judged within those renders, so none of its frames is shown.
//! - A run ends when a frame does not follow on. When nothing follows for [`hold_ns`] its held frame is shown (a
//!   step or the end of a scrub), but the run is still judged if it carries on.
//!
//! - **Fragments of a fill.** Resolve fills its cache from two threads: every so often one renders a few frames
//!   a little way back, which cuts the fill's run short. A run that starts within [`FRAGMENT_GAP_NS`] of a
//!   frame judged to be a fill, and ends without being verified as playback, was part of the fill too: its held
//!   frame is not shown.
//!
//! The only delay the gate adds is at the start of a run: five renders for a drag or playback (up to
//! [`verify_ns`], three periods, at least 60 ms, for a slow one), [`hold_ns`] (a period, at least 40 ms) for a
//! single step.
//!
//! Known limit: a cache fill that renders no faster than about 12.5 ms a render (heavy grades, UHD at 50
//! or 60 fps) cannot be told from a scrub or playback by pacing; it is shown. Telling them apart there needs the playhead,
//! which only the scripting side can supply (ticket #16).

/// A cache fill is recognised once its run is at least this many frames long.
pub const MIN_CACHE_FILL_RUN: u64 = 4;
/// A cache fill averages less than this per render (and less than 0.83 of a period). Scrubs on this machine
/// render no faster than about 14 ms a render at UHD (#104).
pub const FILL_INTERVAL_NS: u64 = 12_500_000;
/// The longest gap inside a run (Resolve stalls about 50 ms now and then at 60 fps).
pub const MAX_HITCH_NS: u64 = 250_000_000;
/// A run that starts this soon after a fill's frame, and is never verified as playback, is part of the fill.
pub const FRAGMENT_GAP_NS: u64 = 100_000_000;
/// How many of a run's newest frames its pace is judged on.
pub const WINDOW: usize = 8;

/// What the gate needs to know about a frame.
#[derive(Clone, Copy, Debug)]
pub struct Timing {
    pub sender_instance: u32,
    pub frame_time: f64,
    pub frame_rate: f64,
    pub render_ns: u64,
}

/// Counts of what the gate has decided so far.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct GateCounts {
    /// Distinct frames shown.
    pub viewer_frames: u64,
    /// Renders of a frame that had just been rendered.
    pub repeat_renders: u64,
    /// Renders kept out of the Picture View because they were, or looked like the start of, a cache fill.
    pub cache_renders_ignored: u64,
}

fn period_ns(frame_rate: f64) -> u64 {
    (1e9 / if frame_rate >= 1.0 { frame_rate } else { 24.0 }) as u64
}

/// How long a new run is watched before it counts as playback.
pub fn verify_ns(period: u64) -> u64 {
    (3 * period).max(60_000_000)
}

/// How long a run waits for its next frame before it is over.
pub fn hold_ns(period: u64) -> u64 {
    period.max(40_000_000)
}

#[derive(Clone, Copy, PartialEq, Debug)]
enum State {
    /// New: its newest frame is held back.
    Verifying,
    /// Playback (or a user's run already shown): every frame shown at once.
    Showing,
    /// A cache fill: nothing shown.
    Fill,
}

struct Run {
    instance: u32,
    period: u64,
    first_time: f64,
    first_ns: u64,
    /// The run's clock: the earliest of (render time minus frames since the run's first frame times the period)
    /// over the run's first frame and the frames older than the recent window. A frame more than a period below it is ahead of the clock.
    clock: f64,
    /// (frame number, render time) of the newest frames, oldest first, at most [`WINDOW`].
    recent: std::collections::VecDeque<(f64, u64)>,
    last_time: f64,
    last_ns: u64,
    /// +1 or -1 once known.
    direction: f64,
    state: State,
    /// Started right after a fill's frame: unless it is verified as playback, it is part of the fill.
    fragment: bool,
}

impl Run {
    /// Does a frame follow on from this run?
    fn continues(&self, t: &Timing) -> bool {
        let step = (t.frame_time - self.last_time)
            * if self.direction == 0.0 {
                1.0
            } else {
                self.direction
            };
        let step = if self.direction == 0.0 {
            step.abs()
        } else {
            step
        };
        t.sender_instance == self.instance
            && (1.0..=3.0).contains(&step)
            && t.render_ns >= self.last_ns
            && t.render_ns - self.last_ns <= (2 * self.period).max(MAX_HITCH_NS)
    }

    /// Frames covered and time taken by the recent frames.
    fn recent_pace(&self) -> (f64, f64) {
        let (&(t0, n0), &(t1, n1)) = (self.recent.front().unwrap(), self.recent.back().unwrap());
        ((t1 - t0).abs(), n1.saturating_sub(n0) as f64)
    }

    fn offset(&self, frame_time: f64, render_ns: u64) -> f64 {
        render_ns as f64 - (frame_time - self.first_time).abs() * self.period as f64
    }

    /// Are the recent frames coming faster than any playback, and ahead of the run's clock?
    fn is_fill(&self) -> bool {
        let (frames, elapsed) = self.recent_pace();
        let renders = (self.recent.len() - 1) as f64;
        let limit = (self.period as f64 * 0.83).min(FILL_INTERVAL_NS as f64);
        // The pace is per render, not per frame number: a scrub jumps three frames at a time but cannot render
        // faster than the GPU allows, as a fill (one frame per render) does.
        frames >= (MIN_CACHE_FILL_RUN - 1) as f64
            && renders >= (MIN_CACHE_FILL_RUN - 1) as f64
            && elapsed < renders * limit
            && self.offset(self.last_time, self.last_ns) < self.clock - self.period as f64
    }

    /// Have the last [`WINDOW`] frames come at the frame rate, one at a time? A fill renders in pairs (two
    /// frames under a few milliseconds apart) even where its average pace drops to real time; playback does not.
    fn is_steady(&self) -> bool {
        let (frames, elapsed) = self.recent_pace();
        self.recent.len() == WINDOW
            && elapsed >= frames * 0.9 * self.period as f64
            && self.one_at_a_time()
    }

    /// No two of the recent frames came less than a quarter period apart.
    fn one_at_a_time(&self) -> bool {
        self.recent
            .iter()
            .zip(self.recent.iter().skip(1))
            .all(|(a, b)| b.1.saturating_sub(a.1) >= self.period / 4)
    }

    fn push(&mut self, frame_time: f64, render_ns: u64) {
        self.last_time = frame_time;
        self.last_ns = render_ns;
        if self.recent.len() == WINDOW
            && let Some((t, n)) = self.recent.pop_front()
        {
            self.clock = self.clock.min(self.offset(t, n));
        }
        self.recent.push_back((frame_time, render_ns));
    }
}

/// Holds at most one frame of type `T` back at a time.
pub struct ViewerGate<T> {
    run: Option<Run>,
    /// The newest frame of a run being verified.
    held: Option<T>,
    /// Render time of the newest frame judged to be part of a cache fill.
    last_fill_ns: Option<u64>,
    counts: GateCounts,
}

impl<T> Default for ViewerGate<T> {
    fn default() -> Self {
        ViewerGate {
            run: None,
            held: None,
            last_fill_ns: None,
            counts: GateCounts::default(),
        }
    }
}

impl<T> ViewerGate<T> {
    pub fn counts(&self) -> GateCounts {
        self.counts
    }

    /// Forget the pacing history and let go of any frame held back (the Feed or the driving Sender changed).
    pub fn reset(&mut self) {
        self.run = None;
        self.held = None;
        self.last_fill_ns = None;
    }

    /// A frame arrived from the Feed. Frames to show are pushed onto `show`, oldest first.
    pub fn offer(&mut self, timing: Timing, frame: T, _now_ns: u64, show: &mut Vec<T>) {
        if let Some(run) = &mut self.run
            && run.instance == timing.sender_instance
            && run.last_time == timing.frame_time
        {
            self.counts.repeat_renders += 1;
            run.last_ns = run.last_ns.max(timing.render_ns);
            match run.state {
                State::Verifying => self.held = Some(frame),
                State::Showing => show.push(frame),
                State::Fill => self.counts.cache_renders_ignored += 1,
            }
            return;
        }

        if self.run.as_ref().is_some_and(|r| r.continues(&timing)) {
            let run = self.run.as_mut().unwrap();
            if run.direction == 0.0 {
                run.direction = (timing.frame_time - run.last_time).signum();
            }
            run.push(timing.frame_time, timing.render_ns);
            if run.state == State::Fill && run.is_steady() {
                run.state = State::Showing; // it settled into playback
            }
            if run.state != State::Fill && run.is_fill() {
                // Every frame of the run so far (none of them shown, as it was still being verified) was the fill.
                if run.state == State::Verifying {
                    self.held = None;
                    self.counts.cache_renders_ignored +=
                        (run.last_time - run.first_time).abs() as u64;
                }
                run.state = State::Fill;
            }
            match run.state {
                State::Fill => {
                    self.counts.cache_renders_ignored += 1;
                    self.last_fill_ns = Some(timing.render_ns);
                }
                State::Showing => {
                    self.counts.viewer_frames += 1;
                    show.push(frame);
                }
                State::Verifying => {
                    // A newer frame of the same run replaces the one held back (not counted: it may be playback).
                    self.held = Some(frame);
                    if run.last_ns - run.first_ns >= verify_ns(run.period) && run.one_at_a_time() {
                        run.state = State::Showing; // playback
                        self.release(show);
                    } else if !run.fragment && run.recent.len() as u64 > MIN_CACHE_FILL_RUN {
                        // A drag: a run that has gone more renders than a cache fill is judged on, and was not
                        // judged one (above), is the user following the playhead, and every frame is shown.
                        run.state = State::Showing;
                        self.release(show);
                    }
                }
            }
            return;
        }

        self.end_run(show);
        let period = period_ns(timing.frame_rate);
        let fragment = self
            .last_fill_ns
            .is_some_and(|f| timing.render_ns.saturating_sub(f) <= FRAGMENT_GAP_NS);
        self.run = Some(Run {
            instance: timing.sender_instance,
            period,
            first_time: timing.frame_time,
            first_ns: timing.render_ns,
            recent: std::collections::VecDeque::from([(timing.frame_time, timing.render_ns)]),
            clock: timing.render_ns as f64,
            last_time: timing.frame_time,
            last_ns: timing.render_ns,
            direction: 0.0,
            state: State::Verifying,
            fragment,
        });
        self.held = Some(frame);
    }

    /// Time has passed (`now_ns`, the same clock as `render_ns`): a run with nothing new for a while is over.
    pub fn tick(&mut self, now_ns: u64, show: &mut Vec<T>) {
        if self.held.is_some()
            && self.run.as_ref().is_some_and(|r| {
                r.state == State::Verifying && now_ns >= r.last_ns + hold_ns(r.period)
            })
        {
            // The run paused without being a cache fill: the user stepped or scrubbed. Show where it got to. The
            // run is still watched: if it carries on it is held and judged again.
            self.release_or_drop_fragment(show);
        }
    }

    fn release(&mut self, show: &mut Vec<T>) {
        if let Some(frame) = self.held.take() {
            self.counts.viewer_frames += 1;
            show.push(frame);
        }
    }

    /// A frame that does not follow on has arrived: the current run is over.
    fn end_run(&mut self, show: &mut Vec<T>) {
        if self
            .run
            .as_ref()
            .is_some_and(|r| r.state == State::Verifying)
        {
            self.release_or_drop_fragment(show); // a run the user made, cut short: show where it got to
        }
        self.run = None;
    }

    /// A run ended or paused before it was verified: show its held frame, unless it was a fragment of a fill.
    fn release_or_drop_fragment(&mut self, show: &mut Vec<T>) {
        match &self.run {
            Some(r) if r.fragment => {
                if self.held.take().is_some() {
                    self.counts.cache_renders_ignored += 1;
                }
                self.last_fill_ns = Some(r.last_ns); // the fill carries on through its fragments
            }
            _ => self.release(show),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 277 Feed frames of the scrub recorded in Resolve 21.2 (UHD, 50 fps, ticket #103): a slow drag one frame
    /// every 400 ms and more, a fast drag forwards, backwards and forwards again at about 15 ms a render, and a
    /// stop mid-drag. One line per frame: `seq instance time rate render_us read_us`, the two times in
    /// microseconds from the first frame. (The whole log, 6004 lines, stays outside the repo.)
    const EXCERPT: &str = "\
751 4 1366 50 0 7896
752 4 1365 50 136471 144098
753 4 1364 50 1597453 1605679
754 4 1363 50 1837969 1845790
755 4 1362 50 1924760 1932514
756 4 1361 50 1984124 1992044
757 4 1362 50 3003223 3011746
758 4 1363 50 3015945 3024503
759 4 1366 50 3031017 3039361
760 4 1369 50 3049314 3057402
761 4 1372 50 3063182 3071217
762 4 1376 50 3078227 3086031
763 4 1379 50 3093172 3101898
764 4 1383 50 3107061 3115679
765 4 1386 50 3123057 3131570
766 4 1389 50 3137099 3145312
767 4 1392 50 3163063 3171799
768 4 1395 50 3181464 3190917
769 4 1397 50 3195636 3204795
770 4 1401 50 3209570 3219704
771 4 1404 50 3224907 3234619
772 4 1407 50 3240100 3249529
773 4 1410 50 3255481 3264408
774 4 1413 50 3270493 3280428
775 4 1416 50 3285959 3295246
776 4 1418 50 3299989 3310198
777 4 1422 50 3315404 3325082
778 4 1425 50 3331672 3342147
779 4 1430 50 3347559 3357107
780 4 1433 50 3360234 3369854
781 4 1439 50 3376819 3385731
782 4 1443 50 3390874 3400575
783 4 1448 50 3406165 3415442
784 4 1452 50 3421468 3431307
785 4 1456 50 3436796 3446152
786 4 1459 50 3450706 3459962
787 4 1462 50 3466046 3475842
788 4 1464 50 3481225 3490672
789 4 1465 50 3495362 3503384
790 4 1466 50 3503210 3512904
791 4 1467 50 3524338 3533010
792 4 1468 50 3537119 3545674
793 4 1469 50 3564991 3573138
794 4 1470 50 3572807 3582679
795 4 1471 50 3595080 3604902
796 4 1473 50 3610443 3618673
797 4 1474 50 3618269 3628212
798 4 1475 50 3639612 3648311
799 4 1476 50 3647420 3656778
800 4 1475 50 3697844 3706431
801 4 1474 50 3711780 3720189
802 4 1472 50 3726812 3735022
803 4 1469 50 3740715 3748794
804 4 1466 50 3758872 3766790
805 4 1461 50 3774023 3782672
806 4 1455 50 3786943 3796441
807 4 1448 50 3802892 3811269
808 4 1443 50 3818060 3826092
809 4 1435 50 3843741 3853609
810 4 1430 50 3860184 3869504
811 4 1423 50 3875744 3885383
812 4 1418 50 3892002 3901324
813 4 1415 50 3904082 3913006
814 4 1409 50 3919374 3928907
815 4 1405 50 3934498 3943747
816 4 1399 50 3949740 3959639
817 4 1394 50 3965148 3974466
818 4 1388 50 3978261 3988199
819 4 1383 50 3993608 4003012
820 4 1379 50 4008763 4017849
821 4 1372 50 4022931 4032678
822 4 1368 50 4039185 4048620
823 4 1363 50 4052376 4061405
824 4 1359 50 4067652 4077356
825 4 1355 50 4082853 4092256
826 4 1353 50 4098114 4107191
827 4 1349 50 4110598 4119954
828 4 1347 50 4126612 4135906
829 4 1345 50 4143114 4152915
830 4 1342 50 4157168 4166728
831 4 1340 50 4173622 4182619
832 4 1337 50 4189848 4198591
833 4 1335 50 4204075 4213491
834 4 1332 50 4217087 4226295
835 4 1329 50 4233350 4242322
836 4 1327 50 4249563 4258283
837 4 1325 50 4257964 4266784
838 4 1322 50 4276826 4285902
839 4 1320 50 4293179 4302965
840 4 1318 50 4308320 4317848
841 4 1315 50 4321451 4332746
842 4 1314 50 4336583 4346587
843 4 1310 50 4352958 4362623
844 4 1307 50 4367079 4376494
845 4 1305 50 4382467 4391403
846 4 1302 50 4396441 4405221
847 4 1300 50 4404630 4414735
848 4 1297 50 4425740 4434967
849 4 1294 50 4441013 4449844
850 4 1291 50 4455091 4464684
851 4 1288 50 4471362 4480678
852 4 1286 50 4486539 4495598
853 4 1284 50 4501924 4510525
854 4 1283 50 4515959 4524401
855 4 1282 50 4524180 4532855
856 4 1282 50 4532850 4542404
857 4 1280 50 4557974 4566731
858 4 1279 50 4579400 4587904
859 4 1278 50 4587228 4595327
860 4 1277 50 4625386 4634421
861 4 1276 50 4633469 4642898
862 4 1275 50 4654140 4661900
863 4 1274 50 4692603 4700993
864 4 1275 50 4766783 4774461
865 4 1276 50 4781669 4790405
866 4 1278 50 4796696 4804178
867 4 1282 50 4815890 4824320
868 4 1284 50 4828884 4837033
869 4 1288 50 4844939 4852909
870 4 1292 50 4868681 4877299
871 4 1296 50 4877099 4885795
872 4 1299 50 4899914 4909128
873 4 1303 50 4917308 4926192
874 4 1306 50 4932480 4942218
875 4 1311 50 4946795 4956161
876 4 1314 50 4960979 4971066
877 4 1318 50 4977245 4986985
878 4 1322 50 4991302 5001852
879 4 1324 50 5004260 5013532
880 4 1327 50 5021638 5030581
881 4 1330 50 5034531 5044437
882 4 1333 50 5049697 5059351
883 4 1336 50 5065891 5075339
884 4 1340 50 5081274 5090303
885 4 1343 50 5095425 5105178
886 4 1347 50 5111751 5121180
887 4 1350 50 5126819 5136116
888 4 1353 50 5143274 5152110
889 4 1355 50 5157663 5167027
890 4 1358 50 5169824 5178736
891 4 1360 50 5185841 5194678
892 4 1362 50 5200151 5209600
893 4 1364 50 5215462 5225588
894 4 1366 50 5230956 5240514
895 4 1369 50 5245123 5253301
896 4 1371 50 5259125 5268213
897 4 1373 50 5274385 5283102
898 4 1374 50 5282203 5291601
899 4 1375 50 5302357 5310741
900 4 1376 50 5310406 5320305
901 4 1378 50 5330480 5339446
902 4 1380 50 5347944 5357741
903 4 1382 50 5363053 5371620
904 4 1385 50 5379468 5388682
905 4 1387 50 5394877 5404622
906 4 1390 50 5406130 5415289
907 4 1393 50 5421940 5431294
908 4 1396 50 5438302 5447274
909 4 1398 50 5452409 5461137
910 4 1402 50 5466519 5476117
911 4 1404 50 5483050 5492124
912 4 1406 50 5496935 5506033
913 4 1410 50 5511213 5520962
914 4 1413 50 5527411 5536975
915 4 1417 50 5542746 5551905
916 4 1420 50 5557992 5567900
917 4 1424 50 5571097 5580685
918 4 1426 50 5587439 5596684
919 4 1429 50 5601538 5610524
920 4 1431 50 5614476 5623302
921 4 1433 50 5630773 5640465
922 4 1435 50 5646036 5655312
923 4 1438 50 5660109 5670136
924 4 1440 50 5674324 5683901
925 4 1443 50 5690739 5699809
926 4 1446 50 5705999 5714617
927 4 1448 50 5719856 5729495
928 4 1451 50 5735116 5744425
929 4 1453 50 5749182 5758273
930 4 1455 50 5765463 5775274
931 4 1458 50 5780544 5790176
932 4 1460 50 5794734 5803973
933 4 1463 50 5808782 5818857
934 4 1465 50 5823970 5833855
935 4 1467 50 5839545 5847741
936 4 1470 50 5855677 5864820
937 4 1472 50 5869932 5878684
938 4 1474 50 5883092 5891490
939 4 1476 50 5898274 5906387
940 4 1477 50 5906340 5915923
941 4 1478 50 5926341 5934038
942 4 1479 50 5934129 5943619
943 4 1480 50 5956470 5964789
944 4 1479 50 6034791 6043019
945 4 1478 50 6049725 6057890
946 4 1476 50 6064768 6072733
947 4 1474 50 6080910 6088621
948 4 1471 50 6096906 6104517
949 4 1468 50 6111937 6120383
950 4 1465 50 6126916 6135200
951 4 1460 50 6141841 6150037
952 4 1458 50 6155878 6163790
953 4 1455 50 6170879 6179678
954 4 1451 50 6184724 6193453
955 4 1449 50 6200769 6209309
956 4 1445 50 6226633 6235855
957 4 1443 50 6243158 6252921
958 4 1438 50 6256253 6265681
959 4 1435 50 6272630 6282754
960 4 1431 50 6287776 6297620
961 4 1427 50 6304142 6313605
962 4 1424 50 6316000 6325310
963 4 1419 50 6332029 6341220
964 4 1415 50 6345128 6355017
965 4 1410 50 6361429 6371028
966 4 1407 50 6377449 6386983
967 4 1403 50 6392717 6401920
968 4 1400 50 6406988 6415742
969 4 1396 50 6422165 6431734
970 4 1394 50 6437387 6446678
971 4 1390 50 6451484 6461624
972 4 1388 50 6465827 6475502
973 4 1386 50 6480865 6490422
974 4 1382 50 6495018 6504318
975 4 1379 50 6510165 6519278
976 4 1376 50 6526666 6536331
977 4 1373 50 6540066 6549139
978 4 1370 50 6554853 6564017
979 4 1368 50 6569138 6578973
980 4 1365 50 6583306 6592846
981 4 1362 50 6598505 6607738
982 4 1359 50 6613711 6622620
983 4 1356 50 6628851 6638578
984 4 1354 50 6643081 6652445
985 4 1350 50 6658502 6668518
986 4 1348 50 6673525 6682350
987 4 1345 50 6690102 6699441
988 4 1342 50 6702282 6712231
989 4 1339 50 6718085 6727194
990 4 1337 50 6734360 6744248
991 4 1335 50 6749820 6759146
992 4 1333 50 6762805 6771865
993 4 1331 50 6777887 6786790
994 4 1330 50 6785910 6795280
995 4 1328 50 6807001 6816546
996 4 1327 50 6822293 6830404
997 4 1326 50 6830169 6838895
998 4 1325 50 6838985 6848474
999 4 1324 50 6865442 6874957
1000 4 1322 50 6881677 6889849
1001 4 1321 50 6889745 6899416
1002 4 1320 50 6911594 6920602
1003 4 1319 50 6919671 6929089
1004 4 1318 50 6942678 6951303
1005 4 1317 50 6950898 6959799
1006 4 1316 50 6972854 6980960
1007 4 1315 50 6980722 6988413
1008 4 1314 50 7008134 7016965
1009 4 1314 50 7016186 7025461
1010 4 1314 50 7025440 7033935
1011 4 1312 50 7058808 7066733
1012 4 1311 50 7066430 7075467
1013 4 1310 50 7094282 7103353
1014 4 1309 50 7102394 7113153
1015 4 1308 50 7128848 7140152
1016 4 1307 50 7139316 7150047
1017 4 1306 50 7166129 7176853
1018 4 1305 50 7182518 7192788
1019 4 1304 50 7216847 7224520
1020 4 1303 50 7235143 7243674
1021 4 1302 50 7274413 7282730
1022 4 1301 50 7305323 7313784
1023 4 1445 50 8268793 8277742
1024 4 1444 50 8276857 8286247
1025 4 1443 50 8301144 8310618
1026 4 1441 50 8316483 8325579
1027 4 1439 50 8332745 8343755
";

    struct Ev {
        seq: u64,
        instance: u32,
        time: f64,
        rate: f64,
        render_ns: u64,
        read_ns: u64,
    }

    fn parse(log: &str) -> Vec<Ev> {
        log.lines()
            .map(|l| {
                let w: Vec<&str> = l.split_whitespace().collect();
                Ev {
                    seq: w[0].parse().unwrap(),
                    instance: w[1].parse().unwrap(),
                    time: w[2].parse().unwrap(),
                    rate: w[3].parse().unwrap(),
                    render_ns: 1_000_000_000_000 + w[4].parse::<u64>().unwrap() * 1000,
                    read_ns: 1_000_000_000_000 + w[5].parse::<u64>().unwrap() * 1000,
                }
            })
            .collect()
    }

    const TICK_NS: u64 = 5_000_000;

    /// Replays frames like the app does: each frame offered when the app read it, the gate ticked every 5 ms in
    /// between. Returns (frame seq, time it was shown at) for every frame shown.
    fn replay(evs: &[Ev]) -> Vec<(u64, u64)> {
        let mut gate = ViewerGate::<u64>::default();
        let (mut shown, mut out) = (Vec::new(), Vec::new());
        let mut now = evs[0].read_ns;
        for e in evs {
            while now + TICK_NS < e.read_ns {
                now += TICK_NS;
                gate.tick(now, &mut out);
                shown.extend(out.drain(..).map(|s| (s, now)));
            }
            now = now.max(e.read_ns);
            let t = Timing {
                sender_instance: e.instance,
                frame_time: e.time,
                frame_rate: e.rate,
                render_ns: e.render_ns,
            };
            gate.offer(t, e.seq, now, &mut out);
            gate.tick(now, &mut out);
            shown.extend(out.drain(..).map(|s| (s, now)));
        }
        // Let the clock run on so a frame still held at the end is shown by the gate's own timeout.
        for _ in 0..20 {
            now += TICK_NS;
            gate.tick(now, &mut out);
            shown.extend(out.drain(..).map(|s| (s, now)));
        }
        shown
    }

    /// Milliseconds from the app reading frame `i` until it, or a newer frame, is on screen.
    fn latency_ms(evs: &[Ev], shown: &[(u64, u64)], i: usize) -> u64 {
        shown
            .iter()
            .find(|(seq, at)| {
                *at >= evs[i].read_ns && evs.iter().position(|e| e.seq == *seq).unwrap() >= i
            })
            .map_or(u64::MAX, |(_, at)| (at - evs[i].read_ns) / 1_000_000)
    }

    fn render_gap_ms(evs: &[Ev], i: usize) -> u64 {
        if i == 0 {
            0
        } else {
            (evs[i].render_ns - evs[i - 1].render_ns) / 1_000_000
        }
    }

    #[test]
    fn a_scrub_is_followed_at_least_every_frame_period() {
        let evs = parse(EXCERPT);
        let shown = replay(&evs);
        let period_ms = 1000 / evs[0].rate as u64;
        // Every frame the Sender renders during the scrub is on screen, or replaced by a newer one, within four
        // periods: a run is followed from its fifth render on (a fill is judged by then), and a lone frame is
        // held for a period at most. The only frames that wait longer are the stretches rendered 12 ms apart or
        // faster, which the gate judges to be cache fills and keeps hidden.
        let late: Vec<usize> = (0..evs.len())
            .filter(|&i| latency_ms(&evs, &shown, i) > 4 * period_ms)
            .collect();
        let unexplained: Vec<usize> = late
            .iter()
            .copied()
            .filter(|&i| render_gap_ms(&evs, i) > 12 && render_gap_ms(&evs, i + 1) > 12)
            .collect();
        assert!(unexplained.len() <= 3, "frames shown late: {unexplained:?}");
        // And the drag is mostly on screen: the old gate showed 1 frame in 3 of the whole recording.
        assert!(
            shown.len() * 10 >= evs.len() * 7,
            "{} of {} frames shown",
            shown.len(),
            evs.len()
        );
    }

    #[test]
    fn the_stop_frame_of_a_drag_is_shown_at_once() {
        let evs = parse(EXCERPT);
        let shown = replay(&evs);
        // The drag stops at the last frame before the one-second pause.
        let stop = (1..evs.len())
            .filter(|&i| render_gap_ms(&evs, i) > 900)
            .map(|i| i - 1)
            .next_back()
            .unwrap();
        let (seq, at) = shown
            .iter()
            .find(|(seq, _)| *seq >= evs[stop].seq)
            .copied()
            .unwrap();
        assert_eq!(seq, evs[stop].seq, "the stop frame itself is shown");
        let wait = (at - evs[stop].read_ns) / 1_000_000;
        assert!(wait <= 25, "shown {wait} ms after it arrived");
    }

    /// Frames of a synthetic run, `step` frame numbers and `every_ms` apart.
    fn run(first: f64, count: usize, step: f64, every_ms: u64, rate: f64) -> Vec<Ev> {
        (0..count)
            .map(|k| {
                let ns = 1_000_000_000_000 + k as u64 * every_ms * 1_000_000;
                Ev {
                    seq: first as u64 + k as u64,
                    instance: 1,
                    time: first + k as f64 * step,
                    rate,
                    render_ns: ns,
                    read_ns: ns + 1_000_000,
                }
            })
            .collect()
    }

    #[test]
    fn a_cache_fill_stays_hidden() {
        // HD at 24 fps: Resolve fills its cache at 6 ms a frame.
        let shown = replay(&run(1000.0, 100, 1.0, 6, 24.0));
        assert!(shown.is_empty(), "a fill showed {} frames", shown.len());
    }

    #[test]
    fn playback_and_a_single_step_are_shown() {
        // Playback at 25 fps: every frame once verified, the last one included.
        let shown = replay(&run(1000.0, 50, 1.0, 40, 25.0));
        assert!(shown.len() >= 40, "{} of 50 frames shown", shown.len());
        assert_eq!(shown.last().unwrap().0, 1049);
        // One step on its own is held for the hold, then shown.
        let step = run(2000.0, 1, 1.0, 40, 25.0);
        let shown = replay(&step);
        assert_eq!(shown.len(), 1);
        assert!((shown[0].1 - step[0].read_ns) / 1_000_000 <= 50);
    }
}
