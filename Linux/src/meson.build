config_conf = configuration_data()
config_conf.set_quoted('APP_ID', application_id)
config_conf.set_quoted('PROFILE', profile)
config_conf.set_quoted('VERSION', meson.project_version())
config_conf.set_quoted('GETTEXT_PACKAGE', gettext_package)
# The development build reads the compiled translations in build/po/ directly, so all four languages can be tested without installing.
if get_option('profile') == 'development'
  config_conf.set_quoted('LOCALEDIR', meson.project_build_root() / 'po')
else
  config_conf.set_quoted('LOCALEDIR', localedir)
endif
config_conf.set_quoted('RESOURCES_FILE', resources_file)
config_conf.set_quoted('ICON_NAME', base_id)
# The development build has no installed icons: read them from data/icons in the source (for both the app and the system tray).
if get_option('profile') == 'development'
  config_conf.set_quoted('ICON_SEARCH_PATH', meson.project_source_root() / 'data' / 'icons')
else
  config_conf.set_quoted('ICON_SEARCH_PATH', '')
endif
config_file = configure_file(input: 'config.rs.in', output: 'config.rs', configuration: config_conf)

# config.rs stays in the build directory and is passed to cargo through an environment variable (no files generated in the source tree).
cargo_env = {
  'ANEMOBIT_CONFIG': meson.current_build_dir() / 'config.rs',
  # The settings tests read the GSettings schema compiled in the build directory.
  'GSETTINGS_SCHEMA_DIR': meson.project_build_root() / 'data',
}
cargo_options = [
  '--manifest-path', meson.project_source_root() / 'Cargo.toml',
  '--target-dir', meson.project_build_root() / 'cargo-target',
]
if get_option('profile') == 'default'
  cargo_options += ['--release']
  rust_target = 'release'
else
  rust_target = 'debug'
endif

cargo_build = custom_target(
  'cargo-build',
  build_by_default: true,
  build_always_stale: true,
  output: meson.project_name(),
  console: true,
  install: true,
  install_dir: bindir,
  depends: resources,
  env: cargo_env,
  command: [
    cargo, 'build', cargo_options,
    '&&',
    'cp', meson.project_build_root() / 'cargo-target' / rust_target / meson.project_name(), '@OUTPUT@',
  ],
)

test(
  'cargo-test',
  cargo,
  args: ['test', '--workspace', cargo_options],
  env: cargo_env,
  depends: [resources, compiled_schemas],
  timeout: 600,
)

# Clippy warnings are also errors (workspace lints), so make test checks them too.
test(
  'cargo-clippy',
  cargo,
  args: ['clippy', '--workspace', '--all-targets', cargo_options],
  env: cargo_env,
  # Package builds skip this suite: Fedora ships clippy separately from cargo.
  suite: 'lint',
  depends: resources,
  timeout: 600,
)

# make run: launch with the build directory's schema when not installed.
devenv = environment()
devenv.set('GSETTINGS_SCHEMA_DIR', meson.project_build_root() / 'data')
meson.add_devenv(devenv)
run_target(
  'run',
  command: [cargo_build],
  env: {'GSETTINGS_SCHEMA_DIR': meson.project_build_root() / 'data'},
  depends: compiled_schemas,
)
