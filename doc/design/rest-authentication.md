# REST Authentication

## References

- https://auth0.com/blog/build-an-api-in-rust-with-jwt-authentication-using-actix-web/
- https://jwt.io/
- https://russelldavies.github.io/jwk-creator/
- https://blog.logrocket.com/how-to-secure-a-rest-api-using-jwt-7efd83e71432/
- https://blog.logrocket.com/jwt-authentication-in-rust/

## Overview

The [REST API][REST] provides a means of controlling Mayastor. It allows the consumer of the API to perform operations
such as creation and deletion of pools, replicas, nexus and volumes.

It is important to secure the [REST] API to prevent access to unauthorised personnel. This is achieved through the use
of
[JSON Web Tokens (JWT)][JWT] which are sent with every [REST] request.

Upon receipt of a request the [REST] server extracts the [JWT] and verifies its authenticity. If authentic, the request
is
allowed to proceed otherwise the request is failed with an [HTTP] `401` Unauthorized error.

## JSON Web Token (JWT)

Definition taken from here:

> JSON Web Token ([JWT]) is an open standard ([RFC 7519][JWT]) that defines a compact and self-contained way for
> securely transmitting information between parties as a JSON object. \
> This information can be verified and trusted because it is digitally signed. \
> [JWT]s can be signed using a secret (with the [HMAC] algorithm) or a public/private key pair using [RSA] or
> [ECDSA].

The [REST] server expects the [JWT] to be signed with a private key and for the public key to be accessible as
a [JSON Web Key (JWK)][JWK].

The JWK is used to authenticate the [JWT] by checking that it was indeed signed by the corresponding private key.

The [JWT] comprises three parts, each separated by a fullstop:

`<header>.<payload>.<signature>`

Each of the above parts are [Base64-URL] encoded strings.

## JSON Web Key (JWK)

Definition taken from here:

> A [JSON] Web Key ([JWK]) is a JavaScript Object Notation ([JSON - RFC 7159][JSON]) data structure that represents a
> cryptographic key.

An example of the [JWK] structure is shown below:

```json
{
  "kty": "RSA",
  "n": "tTtUE2YgN2te7Hd29BZxeGjmagg0Ch9zvDIlHRjl7Y6Y9Gankign24dOXFC0t_3XzylySG0w56YkAgZPbu-7NRUbjE8ev5gFEBVfHgXmPvFKwPSkCtZG94Kx-lK_BZ4oOieLSoqSSsCdm6Mr5q57odkWghnXXohmRgKVgrg2OS1fUcw5l2AYljierf2vsFDGU6DU1PqeKiDrflsu8CFxDBAkVdUJCZH5BJcUMhjK41FCyYImtEb13eXRIr46rwxOGjwj6Szthd-sZIDDP_VVBJ3bGNk80buaWYQnojtllseNBg9pGCTBtYHB-kd-NNm2rwPWQLjmcY1ym9LtJmrQCXvA4EUgsG7qBNj1dl2NHcG03eEoJBejQ5xwTNgQZ6311lXuKByP5gkiLctCtwn1wGTJpjbLKo8xReNdKgFqrIOT1mC76oZpT3AsWlVH60H4aVTthuYEBCJgBQh5Bh6y44ANGcybj-q7sOOtuWi96sXNOCLczEbqKYpeuckYp1LP",
  "e": "AQAB",
  "alg": "RS256",
  "use": "sig"
}
```

The meaning of these keys (as defined on [RFC 7517][[JWK]]) are:

| Key Name |      Meaning       |                                                                                     Purpose                                                                                     |
|:---------|:------------------:|:-------------------------------------------------------------------------------------------------------------------------------------------------------------------------------:|
| kty      |      Key Type      |                                                                 Denotes the cryptographic algorithm family used                                                                 |
| n        |      Modulus       |                                                                       The modulus used by the public key                                                                        |
| e        |      Exponent      |                                                                       The exponent used by the public key                                                                       |
| alg      | The algorithm used |                                                        This corresponds to the algorithm used to sign/encrypt the [JWT]                                                         |
| use      |   Public Key Use   | Can take one of two values sig or enc. sig indicates the public key should be used only for signature verification, whereas enc denotes that it is used for encrypting the data |

<br>

## REST Server Authentication

### Prerequisites

1. The [JWT] is included in the [HTTP] Authorization Request Header
2. The [JWK], used for signature verification, is accessible

### Process

The [REST] server makes use of the [jsonwebtoken] crate to perform [JWT] authentication.

Upon receipt of a [REST] request the [JWT] is extracted from the header and split into two parts:

1. message (comprising the header and payload)
2. signature

This is passed to the jsonwebtoken crate along with the decoding key and algorithm extracted from the [JWK].

If authentication succeeds the [REST] request is permitted to continue. If authentication fails, the [REST] request is
rejected with an [HTTP] `401` Unauthorized error.

[REST]: https://en.wikipedia.org/wiki/REST

[JWT]: https://datatracker.ietf.org/doc/html/rfc7519

[JWK]: https://datatracker.ietf.org/doc/html/rfc7517

[HTTP]: https://developer.mozilla.org/en-US/docs/Web/HTTP

[Base64-URL]: https://base64.guru/standards/base64url

[HMAC]: https://datatracker.ietf.org/doc/html/rfc2104

[RSA]: https://en.wikipedia.org/wiki/RSA_(cryptosystem)

[ECDSA]: https://en.wikipedia.org/wiki/Elliptic_Curve_Digital_Signature_Algorithm

[JSON]: https://datatracker.ietf.org/doc/html/rfc7159

[jsonwebtoken]: https://github.com/Keats/jsonwebtoken
