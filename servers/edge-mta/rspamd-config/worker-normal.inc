# Rspamd normal worker configuration for Nova Mail

bind_socket = "localhost:11333";
.include "$CONFDIR/worker-normal.inc"

# Enable modules
enabled_modules = ["dkim", "spf", "dmarc", "arc", "fuzzy_check", "chartable", "phishing", "regexp", "rbl"];
