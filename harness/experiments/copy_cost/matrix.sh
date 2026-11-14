#!/bin/bash
# usage: matrix.sh TAG   (TAG distinguishes launches: staged/direct)
TAG=${1:-staged}
cd /tmp/s12
for fps in 23.976 25 50 60; do
  for rep in 1 2; do
    for cfg in off on on_reader; do
      python3 run_play.py $fps $cfg ${TAG}_${fps}_${cfg}_$rep 20 2>&1 | tail -1
    done
  done
done
echo MATRIX DONE
