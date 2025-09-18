# ![warden](res/banner@2x.png)

Warden does two things for your Git workflow: Git profile management and OAuth-based Git credentials.

Best-effort mirrors of this repository are maintained at [codeberg.org](https://codeberg.org/adolar0042/warden) and [git.gay](https://git.gay/adolar0042/warden). No guarantees are provided about their freshness or availability.

## Table of Contents

[Install](#install)
[Quick Start](#quick-start)
[Credential Management](#credential-management)
[Profile Management](#profile-management)
[Configuration](#configuration)
[License](#license)

## Install

Compile and install via Cargo:

```bash
cargo install --git https://github.com/adolar0042/warden
# with vendored libs
cargo install --git https://github.com/adolar0042/warden --features vendored
```

**Note that git credential helpers must be installed in a directory in your PATH with the name `git-credentian-<name>`. `~/.cargo/bin/` often isn't in PATH when used by GUI applications.**
To ensure it works as expected, additionally link it to a standard location (in this example linux)

```bash
sudo ln -s ~/.cargo/bin/warden /usr/bin/git-credential-warden
```

This linking is important! Mainly out of convenience, so you can use `warden` for all commands and git still has its preferred `git-credential-warden` helper available.

To add shell completions, add the following to your shell configuration file (assuming a POSIX compliant shell):

```bash
eval "$(warden completions <shell>)"
```

## Quick Start

### Configure Git to use warden as a Credential Helper

```bash
git config --global credential.helper warden
```

*(Hint: if you didn't link the `warden` binary to a location in PATH with the name `git-credential-warden`, you can put an `!` before `warden` to make git run the command in your shell where the cargo bin directory is likely already added to PATH)*

### Add OAuth Providers

I have already set up OAuth applications for a few git hosts for you to use. (For more explanation please refer to the [configuration section](#configuration) of the README.)
Create a file at `~/.config/warden/oauth.toml` with the following content:

```toml
[providers."github.com"]
type = "github"
client_id = "Ov23li8uFPnowNKmRc1h"
client_secret = "5b364d7edf01e60a2c2c5bfaf51dc7b66f6fb162"

[providers."gitlab.com"]
type = "gitlab"
client_id = "b154e7459101fcfaf18f57fc5a069bc87c0e16f31482f0531272acefcb143f1b"

[providers."codeberg.org"]
type = "forgejo"
client_id = "a52456a6-fb1d-4326-904d-0139f79a3203"

[providers."git.gay"]
type = "forgejo"
client_id = "30202081-7a59-4a55-a22c-29fe6e6ad769"
```

### Configuration via `git config`

You can configure (or override) OAuth providers without editing `oauth.toml` by using specially named git config keys. This works for both global and per‑repository configuration. (For more explanation please refer to the [configuration section](#configuration) of the README.)

```bash
git config --global credential.https://git.example.com.oauthType forgejo
git config --global credential.https://git.example.com.oauthClientId YOUR_CLIENT_ID
```

#### Use a Specific Credential per Path

If you want Git to pick a specific credential for a user/org or even a single repository, enable path-aware credential matching and set a username for that URL:

```toml
# in ~/.gitconfig
[credential]
helper = warden
useHttpPath = true  # include the path when matching credentials

# Prefer this username for a single repo (must match your remote URL)
[credential "https://git.example.com/exampleUser/repo.git"]
username = credential-name

# Or prefer this username for an entire user/org
[credential "https://git.example.com/exampleUser"]
username = credential-name
```

With `useHttpPath = true`, Git passes the full URL (including `/exampleUser/repo`) to the credential helper and respects different usernames for different paths. Warden will then use the configured username to select the matching stored credential (or prompt you to log in for that username if none exists).

## Credential Management

Warden is a fully featured [Git credential helper](https://git-scm.com/docs/gitcredentials).
When Git needs credentials, it calls warden, which looks up a provider for the host in `oauth.toml`, performs OAuth 2.0 (Auth Code + PKCE or Device Code), returns a username, access token and when provided refresh token to Git, and stores tokens safely in your system keyring.

The following commands are explained in more detail below.

To manage credentials, run `warden login` to add a credential for a provider and fetch/store a token, `warden logout [--hostname HOST] [--name CRED]` to remove credentials, `warden refresh [--hostname HOST] [--name CRED]` to renew a token, `warden switch [--hostname HOST] [--name CRED]` to change the active credential, and `warden status` to review configured hosts, credentials, and whether a token exists in the system keyring.

These commands make it easy to switch identities and inspect state without editing files.

### Log In to an OAuth Provider

Run the following command to log in to an OAuth provider:

```bash
warden login
```

You will be prompted to enter a credential name (defaults to "oauth") and select an OAuth provider from those defined in `oauth.toml`. Warden will then perform the OAuth flow and store your access token in the OS keyring.

### Check Your Configured Credentials

To see which credentials you have configured for each OAuth provider, run:

```bash
warden status
```

This will show you the active credential for each host, the available credentials, and whether a token exists for that credential or not.

### Refresh a Credential

If you need to refresh an OAuth token (for example, if it has expired or was otherwise removed from your OS keyring), you can run:

```bash
warden refresh
```

You can specify a hostname and/or credential name to refresh a specific credential:

```bash
warden refresh --hostname <hostname> --name <credential name>
```

### Switch Credentials for an OAuth Provider

If you have multiple credentials for an OAuth provider, you can switch between them using:

```bash
warden switch
```

If you have two credentials for that provider, warden will toggle between them.
Otherwise it will prompt you to select one of the available credentials for that OAuth provider.
You can optionally specify a hostname and/or credential name to switch to a specific credential:

```bash
warden switch --hostname <hostname> --name <credential name>
```

### Log Out of an OAuth Provider

To log out of an OAuth provider and remove the stored token, run:

```bash
warden logout
```

You can specify a hostname and/or credential name to log out of a specific credential:

```bash
warden logout --hostname <hostname> --name <credential name>
```

## Profile Management

Warden allows you to manage multiple Git profiles and apply them to your repositories based on their remote URLs.

### List Available Profiles

To see the available profiles, run:

```bash
warden list
```

What you will see is a list of profiles like this:

```
  default: Your Name <your_name@example.personal.com>
  work: Your Name (Company Inc.) <your_name@example.company.com>
```

### Apply a Profile to a Repository

```bash
profile apply default
# or
profile apply work
# or this to apply the profile based on rules
profile apply
```

### Show a Profile's Configuration

To inspect a profile's configuration, run:

```bash
warden show <profile_name>
```

For example:

```bash
warden show work
```

This will print the profile's configuration in a TOML-like format, showing all the git config entries that will be applied when you use that profile.

## Configuration

Warden looks for configuration files in `$XDG_CONFIG_HOME/warden` or `~/.config/warden` on Linux, and in `~/.config/warden` on other platforms.

The key files are:

- `oauth.toml` for OAuth providers, an optional port override, and the oauth-only setting
- `profiles.toml` for profiles, rules and patterns

### OAuth

What you see below is what the minimal configuration in [Quick Start](#add-oauth-providers) expands to and all possible other options added and documented.

```toml
# Optional, see below for more details
# port = 12346
# oauth_only = true

[providers."github.com"]
client_id = "Ov23li8uFPnowNKmRc1h"
# client secret is usually optional since we use PKCE, but GitHub requires it
client_secret = "5b364d7edf01e60a2c2c5bfaf51dc7b66f6fb162"
auth_url = "https://github.com/login/oauth/authorize"
token_url = "https://github.com/login/oauth/access_token"
# optional, for device flow
device_auth_url = "https://github.com/login/device/code"
scopes = ["repo", "read:org", "write:org", "workflow"]
# "auto", "device" or "authcode", "device" requires device_auth_url to be set
# "auto" will attempt device flow first if supported, then fall back to auth code flow
preferred_flow = "authcode"

# routes can also be relative to the host
[providers."gitlab.com"]
client_id = "b154e7459101fcfaf18f57fc5a069bc87c0e16f31482f0531272acefcb143f1b"
# this will be resolved to 'https://gitlab.com/oauth/authorize'
auth_url = "/oauth/authorize"
token_url = "/oauth/token"
device_auth_url = "/oauth/authorize_device"
scopes = ["read_repository", "write_repository"]
preferred_flow = "authcode"

[providers."codeberg.org"]
client_id = "a52456a6-fb1d-4326-904d-0139f79a3203"
auth_url = "/login/oauth/authorize"
token_url = "/login/oauth/access_token"
scopes = ["write:repository", "read:repository"]
preferred_flow = "authcode"

[providers."git.gay"]
client_id = "30202081-7a59-4a55-a22c-29fe6e6ad769"
auth_url = "/login/oauth/authorize"
token_url = "/login/oauth/access_token"
scopes = ["write:repository", "read:repository"]
preferred_flow = "authcode"
```

#### Configure or Override Providers via `git config`

You can configure (or override) OAuth providers without editing `oauth.toml` by using specially named git config keys. This works for both global and per‑repository configuration.

```bash
# Global (system/user) provider definition
# When you set oauthType, standard endpoints and defaults are auto-filled
git config --global credential.https://git.example.com.oauthType forgejo
git config --global credential.https://git.example.com.oauthClientId YOUR_CLIENT_ID
# Optional secret (if the provider requires one)
git config --global credential.https://git.example.com.oauthClientSecret YOUR_CLIENT_SECRET
# You may omit the following fields if oauthType is set, include them to override defaults:
# git config --global credential.https://git.example.com.oauthAuthURL /oauth/authorize
# git config --global credential.https://git.example.com.oauthTokenURL /oauth/token
# git config --global credential.https://git.example.com.oauthDeviceAuthURL /oauth/authorize_device
# git config --global credential.https://git.example.com.oauthScopes "read_repository write_repository"
# git config --global credential.https://git.example.com.oauthPreferredFlow authcode
```

To replicate the minimal configuration in [Quick Start](#add-oauth-providers) you would have to run this:

```bash
git config --global credential.helper warden
git config --global credential.https://github.com.oauthType github
git config --global credential.https://github.com.oauthClientId Ov23li8uFPnowNKmRc1h
git config --global credential.https://github.com.oauthClientSecret 5b364d7edf01e60a2c2c5bfaf51dc7b66f6fb162
git config --global credential.https://gitlab.com.oauthType gitlab
git config --global credential.https://gitlab.com.oauthClientId b154e7459101fcfaf18f57fc5a069bc87c0e16f31482f0531272acefcb143f1b
git config --global credential.https://codeberg.org.oauthType forgejo
git config --global credential.https://codeberg.org.oauthClientId a52456a6-fb1d-4326-904d-0139f79a3203
git config --global credential.https://git.gay.oauthType forgejo
git config --global credential.https://git.gay.oauthClientId 30202081-7a59-4a55-a22c-29fe6e6ad769
```

Per‑repository overrides (added via that repo's `.git/config`) automatically take precedence over the same keys defined globally:

```bash
# Inside a repository
git config credential.https://git.example.com.oauthScopes "read_repository"
```

Supported (case‑insensitive) suffixes after `.oauth`:

- `Type` (values: `github`, `gitlab`, `forgejo`, `gitea`)
- `ClientId`
- `ClientSecret`
- `AuthURL`
- `TokenURL`
- `DeviceAuthURL`
- `PreferredFlow`  (values: `auto`, `device`, `authcode`)
- `Scopes` (whitespace or comma separated list, may be omitted or empty)

#### Rules and Behavior

Precedence (later overrides earlier per field)

- `oauth.toml`
- global/system git config
- repo‑local git config

In the git config, `<base>` (between `credential.` and `.oauth...`) can include a scheme (`https://git.example.com`). If endpoint values start with `/`, they are joined to the base (e.g. `/oauth/token` -> `https://git.example.com/oauth/token`).
If `<base>` omits a scheme (e.g. `git.example.com`), `https://` is assumed when joining relative paths.

Invalid or incomplete provider entries (missing `client_id`, invalid URLs, etc) are discarded with a warning, exiting entirely if none are valid.

You can also specify a custom port for the OAuth callback server by adding a `port = 12345` entry in `oauth.toml` or via git config (e.g. `git config --global warden.port 12346`).

#### OAuth-only Mode

If you want to use warden purely as a credential helper without profiles or state (saved credentials), set `oauth-only = true` either in `oauth.toml` or via git config (e.g. `git config --global warden.oauth-only true`). This makes warden stateless, it will not store tokens in the keyring and each Git credential request triggers a fresh OAuth flow.

I recommend adding the git credential cache to your `~/.gitconfig` to avoid having to log in for every Git operation:

```toml
[credential]
helper = cache --timeout=3600
helper = warden
```

The global flag `--device` forces the OAuth Device Authorization Grant for all commands that perform an OAuth flow (`get`, `login` and `refresh`). This is useful in headless or SSH-only environments, or when your browser is on another machine.

Example usages:

```bash
# force device flow during a Git credential lookup
git config --global credential.helper 'warden --device'

# or when running explicit commands
warden --device login
warden --device refresh
```

If the provider does not support device flow (no `device_auth_url` configured), warden will fail with an error.

### Profile Configuration

#### Example Profile Configuration

```toml
[profiles.default]
user.name = "Your Name"
user.email = "your_name@example.personal.com"

[profiles.work]
user.name = "Your Name (Company Inc.)"
user.email = "your_name@example.company.com"

[[rules]]
profile.name = "work"
owner = "Company" # Applies work profile to all repositories in 'Company' org

[[rules]]
profile.name = "work"
host = "example.company.com" # Applies work profile to all repositories with a remote at 'example.company.com'

[[rules]]
profile.name = "work"
repo = "company-project-1" # Applies work profile to all repositories named 'company-project-1'

[[rules]]
profile.name = "default"
```

#### Repository Patterns

Repository patterns let you control how warden parses repository remotes to extract host, owner, and repo for rule matching. Patterns are evaluated top-to-bottom; the first that matches is used. You configure them in `~/.config/warden/profiles.toml` with `[[patterns]]` entries.

Each pattern must define a `regex` with at least a named capture group `repo`. Optional named groups are `host`, `owner`, `scheme`, `user`, and `vcs`. You can also provide defaults for any of these fields directly in the pattern. Two optional fields control rendering:

- `infer = true`: build a canonical URL from the captured/defaulted fields
- `url = "...":` a template to render the URL if `infer` is false, placeholders: `{{vcs}}`, `{{scheme}}`, `{{user}}`, `{{host}}`, `{{owner}}`, `{{repo}}`

Warden ships with defaults that already handle common forms like:

- `git@host:owner/repo(.git)`
- `host:owner/repo`
- `owner/repo`

You usually do not need to change these, but you can add more for custom hosts or layouts.

##### Example Pattern Configuration

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
host = "git.example.com"
owner = "Company"
# though it does not make much sense here, you could also match by repo name:
# repo = "some-repo"
```

## License

This project is licensed under the [GPL-3.0 License](LICENSE.md). See the LICENSE.md file for more information.
