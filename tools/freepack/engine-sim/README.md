# 600 engine sounds from engine-sim

These are the files that render `sound/engine/600/` with [engine-sim](https://github.com/ange-yaghi/engine-sim) (MIT, Ange Yaghi), run headless on Linux.

1. Clone engine-sim with its submodules into `/tmp/claude-0/es/repo` and apply `engine-sim-linux.patch`. The `include/memory_management.h` part goes into the piranha submodule and the `include/sparse_matrix.h` part into simple-2d-constraint-solver.
2. Install `flex libfl-dev libboost-filesystem-dev googletest`. Copy `CMakeLists.txt`, `main.cpp`, `prelude.h` and `synthesizer_nodelta.cpp` to `/tmp/claude-0/es/render`, then build with `CXX=clang++ cmake .. -DFETCHCONTENT_SOURCE_DIR_GOOGLETEST=/usr/src/googletest` in `build3`.
3. Put `gen.py`, `post.py` and `mx600.tmpl` in `/tmp/claude-0/es/work`, then run `build.py` (one 60 s take, ridden on the dyno) and `cut.py` (levelling, limiting and cutting into the game's slots). The released files are `cut.py`'s output at 0.62 gain, with the idles at twice that.

`post.py` levels with a slow, zero-phase gain and limits peaks with a 4 ms look-ahead, so throttle blips don't crack. An earlier fast compressor with soft clipping did make them crack.
