# .desktop
desktop_conf = configuration_data()
desktop_conf.set('icon', base_id)
desktop_file = i18n.merge_file(
  type: 'desktop',
  input: configure_file(
    input: '@0@.desktop.in.in'.format(base_id),
    output: '@BASENAME@',
    configuration: desktop_conf,
  ),
  output: '@0@.desktop'.format(application_id),
  po_dir: podir,
  install: true,
  install_dir: datadir / 'applications',
)
if desktop_file_validate.found()
  test('validate-desktop', desktop_file_validate, args: [desktop_file.full_path()], depends: desktop_file)
endif

# AppStream metainfo
metainfo_conf = configuration_data()
metainfo_conf.set('app-id', application_id)
metainfo_conf.set('gettext-package', gettext_package)
metainfo_file = i18n.merge_file(
  input: configure_file(
    input: '@0@.metainfo.xml.in.in'.format(base_id),
    output: '@BASENAME@',
    configuration: metainfo_conf,
  ),
  output: '@0@.metainfo.xml'.format(application_id),
  po_dir: podir,
  install: true,
  install_dir: datadir / 'metainfo',
)
if appstreamcli.found()
  test(
    'validate-metainfo',
    appstreamcli,
    args: ['validate', '--no-net', '--explain', metainfo_file.full_path()],
    depends: metainfo_file,
  )
endif

# GSettings schema
gschema_conf = configuration_data()
gschema_conf.set('app-id', application_id)
gschema_conf.set('settings-path', settings_path)
gschema_conf.set('gettext-package', gettext_package)
gschema_file = configure_file(
  input: '@0@.gschema.xml.in'.format(base_id),
  output: '@0@.gschema.xml'.format(application_id),
  configuration: gschema_conf,
  install: true,
  install_dir: datadir / 'glib-2.0' / 'schemas',
)
test('validate-gschema', glib_compile_schemas, args: ['--strict', '--dry-run', meson.current_build_dir()])

# Compiled to build/data/ so make run (not installed) can find the schema.
compiled_schemas = custom_target(
  'gschemas.compiled',
  output: 'gschemas.compiled',
  command: [glib_compile_schemas, '--strict', '--targetdir', meson.current_build_dir(), meson.current_build_dir()],
  depend_files: gschema_file,
  build_by_default: true,
)

# Icons: the app icon is a single SVG (scalable); the system tray uses a monochrome SVG (symbolic). Development and release builds share the same icon name.
install_data(
  'icons' / 'hicolor' / 'scalable' / 'apps' / base_id + '.svg',
  install_dir: datadir / 'icons' / 'hicolor' / 'scalable' / 'apps',
)
install_data(
  'icons' / 'hicolor' / 'symbolic' / 'apps' / base_id + '-symbolic.svg',
  install_dir: datadir / 'icons' / 'hicolor' / 'symbolic' / 'apps',
)

subdir('resources')
