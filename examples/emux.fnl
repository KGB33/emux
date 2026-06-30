(local cfg
  {:api-port
   {:locate [(emux.l.envFile ".env" "PORT")
             (emux.l.jsonFile "environment.local.json" ".apiPort")]
    :base "4327"
    :override emux.o.port}
   :db-port
   {:locate [(emux.l.jsonFile "environment.local.json" ".dbPort")]
    :base "5432"
    :override emux.o.port}})

cfg
