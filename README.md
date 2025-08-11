# ![warden](res/banner@2x.png)

Warden does two things for your Git workflow: Git profile management and OAuth-based Git credentials.

## Install

From source, run
```bash
cargo build --release
```
Then copy `target/release/warden` to a directory in your PATH, OR
install it via Cargo.
```bash
cargo install --git https://github.com/adolar0042/warden
# or if locally cloned
cargo install --path .
```
**Note that git credential helpers must be installed in a directory in your PATH. `~/.cargo/bin/` often isn't in PATH when used by GUI applications.**
To ensure it works as expected, additionally link it to a standard location (in this example linux)
```bash
sudo ln -s ~/.cargo/bin/warden /usr/bin/git-credential-warden
```

This linking is important! Mainly out of convenience, so you can use `warden` for all commands and git still has its preferred `git-credential-warden` helper available.

To add shell completions, add the following (or equivalent) to your shell configuration file:
```bash
eval "$(warden completions <shell>)"
```

## Quick start

### Configure Git to use warden as a credential helper

```bash
git config --global credential.helper warden
```
*(Hint: if you didn't link the `warden` binary to a location in PATH with the name `git-credential-warden`, you can put an `!` before `warden` to make git look for an executable named `warden` in your PATH)*

### Add OAuth providers.

Warden looks for configuration files in `$XDG_CONFIG_HOME/warden` or `~/.config/warden` on Linux, and in `~/.config/warden` on other platforms.

The key files are:
- `oauth.toml` for OAuth providers, an optional port override, and the oauth-only setting
- `profiles.toml` for profiles, rules and patterns

I have already set up a GitHub, GitLab and Codeberg OAuth application for you to use.
Create a file at `~/.config/warden/oauth.toml` with the following content:
```toml
# Optional, see below for more details
# port = 12346
# oauth_only = true

[providers."github.com"]
client_id = "Ov23li8uFPnowNKmRc1h"
client_secret = "5b364d7edf01e60a2c2c5bfaf51dc7b66f6fb162"
auth_url = "https://github.com/login/oauth/authorize"
token_url = "https://github.com/login/oauth/access_token"
# optional, for device flow
device_auth_url = "https://github.com/login/device/code"
scopes = ["repo", "read:org", "write:org"]
# "device" or "authcode", "device" requires device_auth_url to be set
preferred_flow = "authcode"

[providers."gitlab.com"]
client_id = "b154e7459101fcfaf18f57fc5a069bc87c0e16f31482f0531272acefcb143f1b"
client_secret = "gloas-92f2da8d9d6831db2880168fb4190c149f3f5a1a25cea80a2c3cde627ae59252"
auth_url = "https://gitlab.com/oauth/authorize"
token_url = "https://gitlab.com/oauth/token"
device_auth_url = "https://gitlab.com/oauth/authorize_device"
scopes = ["read_repository", "write_repository"]
preferred_flow = "authcode"

[providers."codeberg.org"]
client_id = "a52456a6-fb1d-4326-904d-0139f79a3203"
client_secret = "gto_27xza5mffenmko3x3ohojjp6uym52bj6osm7o6a4n6vhevi3euna"
auth_url = "https://codeberg.org/login/oauth/authorize"
token_url = "https://codeberg.org/login/oauth/access_token"
scopes = ["write:repository", "read:repository"]
preferred_flow = "authcode"
```

If you want to use this purely as a credential helper without profiles, you can set `oauth_only = true` in the `oauth.toml` file. This will make warden stateless, meaning it won't store tokens in the keyring. Each Git credential request will trigger a fresh OAuth flow.

I recommend adding the git credential cache to your `~/.gitconfig` to avoid having to log in for every Git operation:
```toml
[credential]
helper = cache --timeout=3600
helper = warden
```
If you *don't* want to use warden purely as a credential helper, read on.

## Credential management

Warden implements the Git credential-helper protocol.
When Git needs credentials, it calls warden, which looks up a provider for the host in `oauth.toml`, performs OAuth 2.0 (Auth Code + PKCE or Device Code), returns a username and access token to Git, and stores tokens safely in your system keyring.

To manage credentials, run `warden login` to add a username for a provider and fetch/store a token, `warden logout [--hostname H] [--name USER]` to remove credentials, `warden refresh [--hostname H] [--name USER]` to renew a token, `warden switch [--hostname H] [--name USER]` to change the active account, and `warden status` to review configured hosts, users, and whether a token exists. (These commands are further explained below.)

These commands make it easy to switch identities and inspect state without editing files.

### Log in to an OAuth provider

Run the following command to log in to an OAuth provider:
```bash
warden login
```
You will be prompted to enter a username (defaults to "oauth") and select an OAuth provider from those defined in `oauth.toml`. Warden will then perform the OAuth flow and store your access token in the OS keyring.

### Check your configured accounts

To see which accounts you have configured for each OAuth provider, run:
```bash
warden status
```
This will show you the active account for each host, the available accounts, and whether a token exists for that account or not.

### Refresh a credential

If you need to refresh an OAuth token (for example, if it has expired or was otherwise removed from your OS keyring), you can run:
```bash
warden refresh
```
You can specify a hostname and username to refresh a specific account:
```bash
warden refresh --hostname github.com --name lena
```

### Switch credentials for an OAuth provider

If you have multiple credentials for an OAuth provider, you can switch between them using:
```bash
warden switch
```
If you have two credentials for that provider, warden will toggle between them.
Otherwise it will prompt you to select one of the available accounts for that OAuth provider.
You can optionally specify a hostname and username to switch to a specific account:
```bash
warden switch --hostname github.com --name lena
```

### Log out of an OAuth provider

To log out of an OAuth provider and remove the stored token, run:
```bash
warden logout
```
You can specify a hostname and username to log out of a specific account:
```bash
warden logout --hostname github.com --name lena
```

## Profile management

Warden allows you to manage multiple Git profiles and apply them to your repositories based on their remote URLs.

### Example profile configuration

```toml
[profiles.default]
user.name = "Your Name"
user.email = "your_name@example.personal.com"

[profiles.work]
user.name = "Your Name (Company Inc.)"
user.email = "your_name@example.company.com"

[[rules]]
profile.name = "work"
owner = "Company" # Applies work profile to all repositories in `Company` org

[[rules]]
profile.name = "default"
```

### List available profiles

To see the available profiles, run:
```bash
warden list
```
What you will see is a list of profiles like this:
```
  default: Your Name <your_name@example.personal.com>
  work: Your Name (Company Inc.) <your_name@example.company.com>
```

### Apply a profile to a repository

```bash
profile apply default
# or
profile apply work
# or this to apply the profile based on rules
profile apply
```

### Show a profile's configuration

To inspect a profile's configuration, run:
```bash
warden show <profile_name>
```
For example:
```bash
warden show work
```
This will print the profile's configuration in a TOML-like format, showing all the git config entries that will be applied when you use that profile.

### Repository patterns

Repository patterns let you control how warden parses repository remotes to extract host, owner, and repo for rule matching. Patterns are evaluated top-to-bottom; the first that matches is used. You configure them in `~/.config/warden/profiles.toml` with `[[patterns]]` entries.

Each pattern must define a `regex` with at least a named capture group `repo`. Optional named groups are `host`, `owner`, `scheme`, `user`, and `vcs`. You can also provide defaults for any of these fields directly in the pattern. Two optional fields control rendering:
- `infer = true`: build a canonical URL from the captured/defaulted fields
- `url = "...":` a template to render the URL if `infer` is false, placeholders: `{{vcs}}`, `{{scheme}}`, `{{user}}`, `{{host}}`, `{{owner}}`, `{{repo}}`

Warden ships with defaults that already handle common forms like:
- `git@host:owner/repo(.git)`
- `host:owner/repo`
- `owner/repo`
You usually do not need to change these, but you can add more for custom hosts or layouts.

Example configuration:
```toml
# Match SSH style like git@github.com:org/repo(.git)
[[patterns]]
regex = '^(?P<user>[0-9A-Za-z\-]+)@(?P<host>[0-9A-Za-z\.\-]+):(?P<owner>[0-9A-Za-z_\.\-]+)/(?P<repo>[0-9A-Za-z_\.\-]+)$'
scheme = "ssh"
infer = true

# Match owner/repo on a specific host over HTTPS
[[patterns]]
regex = '^(?P<owner>[0-9A-Za-z_\.\-]+)/(?P<repo>[0-9A-Za-z_\.\-]+)$'
scheme = "https"
host = "github.com"
infer = true

# Render a custom canonical URL with a template
[[patterns]]
regex = '^(?P<scheme>https)://(?P<host>git\.example\.com)/scm/(?P<owner>.+)/(?P<repo>.+)\.git'
url = 'https://{{host}}/scm/{{owner}}/{{repo}}.git'
```

These parsed components are used by `[[rules]]` to decide which profile to apply, for example:
```toml
[[rules]]
profile.name = "work"
host = "gitlab.com"
owner = "Company"
```

## License

This project is licensed under the [GPLv3 License](LICENSE.md). See the LICENSE file for more information.
