project(
  'anemobit',
  'rust',
  version: '1.0.0',
  license: 'Apache-2.0',
  meson_version: '>= 1.5',
)

i18n = import('i18n')
gnome = import('gnome')
fs = import('fs')

base_id = 'io.github.noctqira.Anemobit'

# The version also appears in Cargo.toml, debian/changelog and build-aux/anemobit.spec; all must match.
# Files missing from the source tree are skipped (the .rpm source tarball has no debian/).
version_files = {
  'Cargo.toml': 'version = "@0@"',
  'debian/changelog': 'anemobit (@0@-',
  'build-aux/anemobit.spec': 'Version:        @0@',
}
foreach file, pattern : version_files
  if fs.is_file(file) and not fs.read(file).contains(pattern.format(meson.project_version()))
    error('The version in @0@ must match meson.build (@1@)'.format(file, meson.project_version()))
  endif
endforeach

dependency('glib-2.0', version: '>= 2.88')
dependency('gio-2.0', version: '>= 2.88')
dependency('gtk4', version: '>= 4.22')
dependency('libadwaita-1', version: '>= 1.9')

cargo = find_program('cargo')
glib_compile_schemas = find_program('glib-compile-schemas')
desktop_file_validate = find_program('desktop-file-validate', required: false)
appstreamcli = find_program('appstreamcli', required: false)

if get_option('profile') == 'development'
  profile = 'Devel'
  application_id = '@0@.@1@'.format(base_id, profile)
else
  profile = ''
  application_id = base_id
endif

# GSettings path: /io/github/noctqira/Anemobit/ (the development build has its own path).
settings_path = '/@0@/'.format(application_id.replace('.', '/'))

gettext_package = meson.project_name()
bindir = get_option('prefix') / get_option('bindir')
datadir = get_option('prefix') / get_option('datadir')
localedir = get_option('prefix') / get_option('localedir')
podir = meson.project_source_root() / 'po'

subdir('data')
subdir('po')
subdir('src')

gnome.post_install(
  glib_compile_schemas: true,
  gtk_update_icon_cache: true,
  update_desktop_database: true,
)
