#!/bin/bash
# Edit the first line (pick → edit), drop the rename commit
sed -i '' '1s/^pick/edit/' "$1"
sed -i '' '/^pick c12b298/d' "$1"
