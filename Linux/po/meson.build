i18n.gettext(gettext_package, preset: 'glib')

# Translations are complete, with no fuzzy entries, and placeholders match the source.
test(
  'check-translations',
  find_program('python3'),
  args: [meson.project_source_root() / 'build-aux' / 'check-translations.py', meson.current_source_dir()],
)
