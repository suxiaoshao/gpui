# HTTP Client bundled GStreamer runtime

HTTP Client itself is distributed under the MIT license. Release packages may
include a private GStreamer 1.28.6 runtime so media response previews work
without a separately installed GStreamer environment.

The runtime is produced only from the upstream sources recorded in
`runtime-manifest.toml`. The macOS and Windows packages come from the official
GStreamer binary distribution. The Linux prefix is built from the pinned
Cerbero revision recorded in that manifest.

Included upstream projects and license families:

- GStreamer core, base/good/bad plugin families and the macOS/Windows runtime
  support libraries: LGPL-2.1-or-later, with individual permissively licensed
  components retained under their upstream terms.
- GLib and related runtime libraries distributed by GStreamer: LGPL-2.1-or-later.
- OpenSSL runtime libraries selected by the official macOS package: Apache-2.0.
- `gst-libav` and its FFmpeg runtime built by the selected upstream distribution:
  LGPL-2.1-or-later configuration. Component packages outside the checked-in
  runtime selection are excluded.
- libogg, libvorbis, libopus, libvpx, libflac and other codec support libraries:
  their upstream BSD-style or permissive licenses.

Upstream source and license material:

- <https://gstreamer.freedesktop.org/src/>
- <https://gitlab.freedesktop.org/gstreamer/gstreamer>
- <https://gitlab.freedesktop.org/gstreamer/cerbero>
- <https://ffmpeg.org/legal.html>
- <https://www.openssl.org/source/>

The packaged runtime includes this notice and the exact source contract used by
the build. No GStreamer SDK or runtime binaries are stored in this Git repository.
