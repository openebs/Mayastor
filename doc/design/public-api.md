# Mayastor Public API

Mayastor exposes a public api from its [REST] service.
This is a [RESTful][REST] API which can be leveraged by external to mayastor (ex: users or 3rd party tools) as well as
mayastor components which are part of the control-plane.

## OpenAPI

The mayastor public API is defined using the [OpenAPI] which has many benefits:

1. Standardized: OpenAPI allows us to define an API in a standard way, well-used in the industry.

2. Integration: As a standard, it's easy to integrate with other systems, tools, and platforms (anyone can write a
   plugin for it!).

3. Automation: Auto generate the server and client libraries, reducing manual effort and the potential for errors.

4. Documentation: Each method and type is documented which makes it easier to understand.

5. Tooling: There's an abundance of tools and libraries which support the OpenAPI spec, making it easier to develop,
   test, and deploy.

The spec is
available [here](https://raw.githubusercontent.com/openebs/mayastor-control-plane/HEAD/control-plane/rest/openapi-specs/v0_api_spec.yaml),
and you interact with it using one of the many ready-made
tools [here](https://editor.swagger.io/?url=https://raw.githubusercontent.com/openebs/mayastor-control-plane/HEAD/control-plane/rest/openapi-specs/v0_api_spec.yaml).

[OpenAPI]: https://www.openapis.org/what-is-openapi

[REST]: https://en.wikipedia.org/wiki/REST
