//! About Anemobit (same as macOS: icon, version, summary, compatibility, website / report an issue / privacy policy / license, © Noctqira).

use adw::prelude::*;
use gettextrs::gettext;

use crate::config;

const WEBSITE: &str = "https://github.com/noctqira/Anemobit";
const ISSUES: &str = "https://github.com/noctqira/Anemobit/issues";
const PRIVACY_POLICY: &str = "https://github.com/noctqira/Anemobit/blob/main/PRIVACY.md";

pub fn show(parent: Option<&impl IsA<gtk::Widget>>) {
    let summary = gettext("Real-time interface traffic of devices over SNMP.");
    let compatibility = gettext(
        "Works with devices that provide the standard IF-MIB with 64-bit interface counters over SNMPv2c or SNMPv3 (authentication: MD5, SHA1, SHA224, SHA256, SHA384, SHA512; privacy: DES, AES-128).",
    );
    let about = adw::AboutDialog::builder()
        .application_name("Anemobit")
        .application_icon(config::ICON_NAME)
        .version(config::VERSION)
        .developer_name("Noctqira")
        .copyright("© 2026 Noctqira")
        .license_type(gtk::License::Apache20)
        .website(WEBSITE)
        .issue_url(ISSUES)
        .comments(format!("{summary}\n\n{compatibility}"))
        .build();
    about.add_link(&gettext("Privacy Policy"), PRIVACY_POLICY);
    about.present(parent);
}
