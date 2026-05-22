{:emux
 {:binding "emux"
  :metadata {:fls/itemKind "Module"
             :fnl/docstring "The emux configuration API. Available as a global in all .fnl config files."}
  :fields
  {:l {:binding "emux.l"
       :metadata {:fls/itemKind "Module"
                  :fnl/docstring "Locator functions — build pipelines that find specific values in project files."}
       :fields
       {:envFile {:binding "emux.l.envFile"
                  :metadata {:fls/itemKind "Function"
                             :fnl/arglist ["path" "variable"]
                             :fnl/docstring "Return a locator that targets `variable` in a dotenv-style file at `path`.

Example:
  (emux.l.envFile \".env\" \"PORT\")"}}
        :files {:binding "emux.l.files"
                :metadata {:fls/itemKind "Function"
                           :fnl/arglist ["glob"]
                           :fnl/docstring "Return a locator that matches files by glob pattern.
Usually chained with emux.l.regex.

Example:
  (emux.l.files \"src/**/*.json\")"}}
        :regex {:binding "emux.l.regex"
                :metadata {:fls/itemKind "Function"
                           :fnl/arglist ["target" "pattern"]
                           :fnl/docstring "Append a regex filter to an existing locator pipeline.

Example:
  (-> (emux.l.files \"*.json\") (emux.l.regex \"4327\"))"}}}}
   :o {:binding "emux.o"
       :metadata {:fls/itemKind "Module"
                  :fnl/docstring "Overrider values — define how located values are replaced."}
       :fields
       {:randPort {:binding "emux.o.randPort"
                   :metadata {:fls/itemKind "Constant"
                              :fnl/docstring "Replace all located values with a randomly generated free port number."}}}}}}}
