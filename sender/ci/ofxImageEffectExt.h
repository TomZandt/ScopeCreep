// CI stand-in for DaVinci Resolve's ofxImageEffectExt.h, which is not in the official OpenFX repository.
// Only what the Sender uses is declared, with the same value as Resolve's header. Used by the Test build
// and Release workflow (.github/workflows/release.yml); a local build uses Resolve's own headers.
#ifndef _ofxImageEffectExt_h_
#define _ofxImageEffectExt_h_

/** Says whether the clip is for thumbnail. Type int x 1, clip instance, read only, 0 or 1. */
#define kOfxImageClipPropThumbnail "kOfxImageClipPropThumbnail"

#endif
