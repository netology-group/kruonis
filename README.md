# Kruonis

[![Build Status][travis-img]][travis]

[travis]:https://travis-ci.com/netology-group/kruonis?branch=master
[travis-img]:https://travis-ci.com/netology-group/kruonis.png?branch=master

Sends empty messages at given intervals.

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
