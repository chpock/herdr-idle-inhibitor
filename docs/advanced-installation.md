# Advanced installation

[Home](../README.md) · [Development](development.md) · [Compatibility](compatibility.md)

For the usual GitHub install, first use, updates and removal, follow the [README](../README.md#get-started). This guide is for binary bundles, revision selection, and installations registered with `herdr plugin link`. To work on the source itself, use [Development](development.md).

All installation methods require Herdr **0.9.3** and a [supported native desktop/OS](compatibility.md). Run as your normal user, without sudo or an administrator service. Installing a different way does not bypass version checks or power-policy limitations. [Hypridle display tuning](hypridle-setup.md) is optional, regardless of the installation method.

## Install a binary bundle

A bundle already contains the native executable and a manifest with no build step. Rust, Cargo, Node.js, Python, jq and power-management wrappers are not needed to run it. GitHub `plugin install` builds from source; it does not download these bundles.

### Find the matching archive

Bundles may be available in the artifacts of successful repository [Actions runs](https://github.com/chpock/herdr-idle-inhibitor/actions). Open a successful run, select the artifact named `herdr-idle-inhibitor-<target>` for your host, and download it. Artifacts expire and are not signed releases; availability is not guaranteed.

| Host | Target / artifact suffix | Inner plugin archive |
| --- | --- | --- |
| Linux x86_64 GNU | `x86_64-unknown-linux-gnu` | `.tar.gz` |
| macOS Apple Silicon | `aarch64-apple-darwin` | `.tar.gz` |
| macOS Intel | `x86_64-apple-darwin` | `.tar.gz` |
| Windows x86_64 MSVC | `x86_64-pc-windows-msvc` | `.zip` |

Linux CI binaries use **glibc 2.39**; older libc compatibility is not promised. The two macOS archives are separate native builds, not a universal binary. There are no Linux or Windows arm64 bundles.

### Verify, extract and link

1. Extract the outer Actions artifact ZIP. Inside it, find `herdr-idle-inhibitor-<version>-<target>.tar.gz` (or `.zip`) and its same-named `.sha256` companion.
2. Verify the plugin archive's SHA-256 against the companion. For example, from the directory containing one downloaded archive and checksum:

   Linux:

   ```sh
   sha256sum -c herdr-idle-inhibitor-*.tar.gz.sha256
   ```

   macOS:

   ```sh
   shasum -a 256 -c herdr-idle-inhibitor-*.tar.gz.sha256
   ```

   On Windows, use PowerShell's `Get-FileHash -Algorithm SHA256` on the inner ZIP and compare its `Hash` with the companion file. A matching checksum detects corruption; it is not a publisher signature or independent proof of trust.

3. Extract the **inner** plugin archive into a persistent directory. Locate the directory containing `herdr-plugin.toml` and `target/release/`; this is what you link, not the outer download directory.
4. If this plugin is already GitHub-installed under the same Herdr configuration directory, [disable and uninstall that copy](../README.md#disable-re-enable-or-remove) before switching installation types. Settings and Pause are outside the installation and remain intact.
5. Link the extracted plugin directory using its absolute path:

   ```sh
   herdr plugin link /absolute/path/to/extracted-plugin --enabled
   ```

   On Windows, use a quoted absolute path such as `"C:\Users\you\Plugins\herdr-idle-inhibitor"`.

6. Follow [opening and checking the plugin](../README.md#2-open-status-and-settings). A bundle has no source-build preparation step, so explicitly opening the popup is the reliable way to activate it in an already-running Herdr session.

Keep the linked directory while the plugin is registered. Preserve the executable layout and generated manifest: replacing it with the source manifest reintroduces a Cargo build requirement. Windows bundle entrypoints explicitly name the `.exe`.

The bundle includes the README, guides, JSON Schema, license, build-environment metadata and `matching-source.tar.gz` with vendored dependencies. Rebuilding that source is described in [Development](development.md#build-the-source-in-a-bundle).

### OS security prompts

macOS bundles are unsigned and unnotarized; quarantine or Gatekeeper may block execution. Windows may show SmartScreen warnings. After inspecting the origin and source, use the OS's intentional per-application approval process. Do not disable system-wide protections. See [distribution and OS security](compatibility.md#distribution-and-os-security).

## Select a GitHub revision

If you intentionally need a particular branch, tag or commit rather than the default branch, supply Herdr's `--ref` option:

```text
herdr plugin install chpock/herdr-idle-inhibitor --ref <branch-tag-or-commit>
```

Herdr builds that revision with its source manifest. It still needs the native source-build toolchain described in the [README requirements](../README.md#requirements). Automatic replacement rejects an executable without the compatible update protocol; installing an old revision is not a supported way to downgrade a running monitor. See [update behavior](behavior.md#automatic-updates).

## Linked installation maintenance

A linked directory is the installation, not a temporary input that Herdr copies elsewhere. There is only one registration for this plugin ID within each Herdr configuration directory. `plugin install` cannot replace a linked installation with a GitHub checkout.

### Update a linked bundle

Extract the replacement bundle into a persistent directory and link that directory with `--enabled`, as above. The resident monitor detects a changed registered directory or executable automatically, preserving settings and Pause. Do not overwrite a running Windows popup's executable in place; close that popup first, or use a new directory.

For a linked source checkout, follow [the rebuild instructions](development.md#change-and-rebuild-a-linked-checkout) instead. Common process-switch behavior and its brief protection gap are described in [How it works](behavior.md#automatic-updates).

### Unlink or change installation type

[Disable the plugin and wait for its monitor to stop](../README.md#disable-re-enable-or-remove), then unlink it:

```sh
herdr plugin unlink herdr-idle-inhibitor
```

You can then remove the linked directory or install another type normally. Preferences and logs are not deleted. An enabled registration in another Herdr configuration directory can keep the shared monitor alive.
