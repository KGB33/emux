(local cfg
  {:api-port
   {:locate [(emux.l.envFile ".env" "PORT")
             (-> (emux.l.files "environment.local.json")
                 (emux.l.regex "4327"))]
    :override emux.o.randPort}})

cfg
