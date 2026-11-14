#!/bin/bash
cd /tmp/s12
for rep in 1 2 3; do for cfg in off on on_reader; do
python3 freerun.py 25 $cfg fr_${1}_${cfg}_$rep 600 mov DNxHRLB 2>&1 | tail -1
done; done
echo FR DONE
