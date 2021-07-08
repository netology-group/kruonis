# Kruonis

Sends empty events at given intervals. Like cron but for mqtt events.

You need to send a `kruonis.subscribe` request to subscribe to these events:

```bash
export SUBSCRIBER=test.joe.usr.example.org KRUONIS=kruonis.dev.svc.example.org
mosquitto.rr -V 5 \ # or mosquitto_rr, depending on your distro
    -i "${SUBSCRIBER}" \
    -t "agents/${SUBSCRIBER}/api/v1/out/${KRUONIS}" \
    -e "agents/${SUBSCRIBER}/api/v1/in/${KRUONIS}" \
    -D connect user-property 'connection_version' 'v2' \
    -D publish user-property 'type' 'request' \
    -D publish user-property 'method' 'kruonis.subscribe' \
    -D publish correlation-data 'foobar' \
    -D publish user-property 'local_timestamp' "$(date +%s000)" \
    -F "%J" \
    -m "{}"
```

then you will receive events in topic:
```
apps/${KRUONIS}/api/v1/events
```

## Deployment

This service has a regular deployment to k8s with skaffold.
For example to deploy the current revision to testing run:

```bash
NAMESPACE=testing ./deploy.init.sh
IMAGE_TAG=$(git rev-parse --short HEAD) skaffold run -n testing
```

## License

The source code is provided under the terms of [the MIT license][license].

[license]:http://www.opensource.org/licenses/MIT
