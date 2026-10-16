# Pinned OCCT release. Kernel upgrades change output topology, so bump only with a full corpus run.
OCCT_VERSION=7.9.3
OCCT_TAG=V7_9_3
# The commit the tag names. build-occt.sh (after its own clone) and occt-source-archive.sh stop when the tag names
# another, so a tag moved upstream changes neither the builds nor the release archive (delivery.md section 2.13).
OCCT_COMMIT=a016080bf6738d6aeae020badee4e888ad1540a5
