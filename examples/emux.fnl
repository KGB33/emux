(local cfg
  {:api-port
   {:locate [(emux.l.envFile ".env" "PORT")
             (-> (emux.l.files "environment.local.json")
                 (emux.l.regex "4327"))]
    :override emux.o.randPort}
   :db-port
   {:locate [(emux.l.jsonFile "environment.local.json" ".dbPort")]
    :override emux.o.randPort}})

cfg
