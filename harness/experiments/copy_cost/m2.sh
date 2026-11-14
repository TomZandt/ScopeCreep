#!/bin/bash
cd /tmp/s12
TAG=$1; shift
for spec in "$@"; do fps=${spec%%:*}; cfg=${spec##*:}
python3 run_play.py $fps $cfg ${TAG}_${fps}_${cfg}_1 20 2>&1 | tail -1
done
echo MATRIX DONE
