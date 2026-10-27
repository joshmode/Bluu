# Third-Party Licenses

Bluu's optional native A2DP component is distributed under GPL-3.0-or-later.
The following components retain their upstream licenses.

| Library | Version / source | License | Use |
|---|---|---|---|
| OpenWinBlue | https://github.com/santiquiroz/openwinblue | GPL-3.0 | A2DP driver/service foundation |
| libldac | https://github.com/anonymix007/libldac | Apache-2.0 | LDAC encoder |
| libsbc | BlueZ SBC | LGPL-2.1-or-later | SBC encoder |
| FFmpeg aptX core | FFmpeg n7.1 `libavcodec/aptx*.c` | LGPL-2.1-or-later | aptX Classic and aptX HD encode core |
| GoogleTest | v1.14.0 via CMake FetchContent | BSD-3-Clause | Host tests only; not shipped |

`libopenaptx` is not included. Its README contains non-OSI restrictions beyond its
GPL file, so it is not suitable for this distribution.

AAC encoding is performed by the Windows Media Foundation encoder supplied by the
operating system; no AAC codec binary is redistributed.

The FFmpeg-derived source is limited to the codec tables and block DSP functions in
`third-party/ffmpeg-aptx`. It is not a build of the FFmpeg framework or its
command-line programs. Preserve the FFmpeg copyright and LGPL notices.
