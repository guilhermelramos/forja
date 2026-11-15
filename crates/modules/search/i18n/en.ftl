search-name = Search
search-panel-title = Search
search-command-focus = Focus search
search-command-run = Search in files
search-method-run-description = Starts a text search across files in the active bench or given folder.
search-method-run-result = Whether the search started.
search-method-results-description = Returns the status and matches of the latest search.
search-method-results-result = Current search status, match count and files.
search-query-hint = Search in files
search-option-regex = Regex
search-option-case = Match case
search-option-word = Whole word
search-include-hint = Include: *.rs, src/**
search-exclude-hint = Exclude: target/**
search-button-search = Search
search-button-stop = Stop
search-button-preview = Preview
search-button-apply = Apply
search-button-confirm = Confirm replace
search-status-idle = Type a term and press Enter.
search-status-running = Searching…
search-status-none = No results.
search-status-results = { $matches ->
    [one] { $matches } match
   *[other] { $matches } matches
} in { $files ->
    [one] { $files } file
   *[other] { $files } files
}
search-status-truncated = Results were capped: narrow the search.
search-status-more = More results are not drawn: narrow the search.
search-replace-title = Replace
search-replace-hint = Replace with
search-preview-summary = { $matches ->
    [one] { $matches } match
   *[other] { $matches } matches
} in { $files ->
    [one] { $files } file
   *[other] { $files } files
}
search-preview-multiline = The change spans lines and is not shown.
search-preview-planning = Computing the preview…
search-notice-no_bench = Open a bench first.
search-notice-empty = Type a term to search.
search-notice-thread = Could not start the search.
search-notice-no_preview = Preview the replacement before applying it.
search-notice-replaced = Replaced { $matches ->
    [one] { $matches } match
   *[other] { $matches } matches
} in { $files ->
    [one] { $files } file
   *[other] { $files } files
}.
search-notice-changed = Nothing was written: these files changed since the preview: { $files }. Preview again.
search-notice-io = Write failed ({ $message }). Written: { $written }; restored: { $restored }.
search-notice-error = Could not finish: { $message }
search-error-invalid-regex = Invalid regular expression.
