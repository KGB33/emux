(local emux
  {:l {:envFile (fn [path variable]
                  {:filters [{:__kind :env_file :path path :variable variable}]})
       :jsonFile (fn [path selector]
                   {:filters [{:__kind :json_file :path path :selector selector}]})
       :files (fn [glob]
                {:filters [{:__kind :file :glob glob}]})
       :regex (fn [target pattern]
                (let [filters []]
                  (each [_ f (ipairs target.filters)]
                    (table.insert filters f))
                  (table.insert filters {:__kind :regex :pattern pattern})
                  {:filters filters}))}
   :o {:randPort {:__kind :random_port}}})

emux
