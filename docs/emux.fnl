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
        :jsonFile {:binding "emux.l.jsonFile"
                   :metadata {:fls/itemKind "Function"
                              :fnl/arglist ["path" "selector"]
                              :fnl/docstring "Return a locator that targets the value at a dotted selector in a JSON file.

Example:
  (emux.l.jsonFile \"env.json\" \".server.port\")"}}}}
   :o {:binding "emux.o"
       :metadata {:fls/itemKind "Module"
                  :fnl/docstring "Overrider values — define how located values are replaced."}
       :fields
       {:port {:binding "emux.o.port"
               :metadata {:fls/itemKind "Constant"
                          :fnl/docstring "Replace all located values with a deterministic per-worktree free-range port."}}}}}}}
