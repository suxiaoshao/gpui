app-title = Novel Download

gpui-form-error-required = This field is required.

download-field-source = Novel source
download-source-placeholder = Enter a novel ID or zgzl.net link
download-source-help = Supports a novel ID, an info link from www.zgzl.net or m.zgzl.net, or a chapter or numbered page link from m.zgzl.net.
download-validation-source-invalid = Enter a supported zgzl.net novel ID or link.

download-action-start = Start download
download-action-cancel = Cancel download

download-state-idle-title = Download a novel
download-state-idle-description = Enter a supported source to download the novel as a text file.
download-state-resolving = Resolving novel metadata…
download-state-downloading = Downloading novel content…
download-state-cancelling = Cancelling download…
download-state-succeeded = Download complete
download-state-failed = Download failed
download-state-cancelled = Download cancelled

download-snapshot-source = Downloading: { $source }
download-progress-novel = { $name } by { $author }
download-progress-items = Downloaded { $count } items
download-progress-current = Current source: { $url }
download-output-path = Saved to: { $path }

download-error-network = The network request failed.
download-error-http-status = The server returned HTTP status { $status }.
download-error-parse = The downloaded novel data could not be parsed.
download-error-range-chapter = The requested chapter was not found.
download-error-range-page = The requested page { $page } was not found.
download-error-output = The downloaded file could not be written.
download-error-download-directory = The system Downloads directory is unavailable.
download-error-target-exists = The target file already exists: { $path }
download-error-staging-exists = A previous incomplete download already exists: { $path }
download-error-cleanup = The incomplete download could not be removed: { $path }
