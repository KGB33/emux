(local emux
  {:l {:envFile (fn [path variable]
                  {:filters [{:__kind :env_file :path path :variable variable}]})
       :jsonFile (fn [path selector]
                   {:filters [{:__kind :json_file :path path :selector selector}]})}
   :o {:randPort {:__kind :random_port}}})

emux
