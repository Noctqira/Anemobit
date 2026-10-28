# Compiled into a single .gresource file, embedded in the executable by src/config.rs with include_bytes!.
resources = gnome.compile_resources(
  'resources',
  'resources.gresource.xml',
  gresource_bundle: true,
)
resources_file = meson.current_build_dir() / 'resources.gresource'
