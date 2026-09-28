# Slide

Slide is Hackflare's lightweight deployment and server management system.

The name comes from sli(m) + de(ploy).

Slide is intended to be a small alternative to platforms such as coolify.
Instead of running a heavy control plane, each server runs a tiny privileged daemon called `slided` which exposes a secure API.

A Slide client can discover every server in a fleet, authenticate to them, deploy services, restart instances, manage rolling releases, inspect state, and update any coniguration.

> [!IMPORTANT]  
> Slide is currently in early design and development. It is for a future Hackflare strategy.

## Goals

Slide is being built around a few ideas:
- Keep the server-side agent small.
- Avoid requiring a large central control plane.
- Make multi-server and multi-region deployments easy and simple.
- support zero-downtime rolling releases, including ports such as port 53.
- Let servers discover and manage each other through DNS.
- Keep configuration and deployment state explicit.
- Make the same system usable from a TUI, GUI, CLI and web interface.
- Avoid tying infrastructure to a specific hosting provider.

## Architecture

Every managed server runs:

```
slided
├── management API
├── container runtime management
├── image deployment
├── health checking
├── rolling deployment proxy
├── logs and metrics
├── secrets/configuration
└── node state
```

Clients conenct directly to slide nodes after discovering them through DNS.

```
              discovery.slide.hackflare.net
                         │
                         │ SVCB
             ┌───────────┼───────────┐
             ▼           ▼           ▼
      one.eu.slide   one.za.slide   two.us.slide
             │           │           │
             └───────────┴───────────┘
                         │
                   Slide client
```

A seperate always-online central controller is not required and is not intended.

## Node naming

Hackflare's Slide deployment will use:
```
<number>.<group>.slide.hackflare.net
```
Groups will normally represent regions or logical infrastructure groups.

An example of our system (the defaults):
```
one.eu.slide.hackflare.net
two.eu.slide.hackflare.net

one.za.slide.hackflare.net

one.us.slide.hackflare.net
two.us.slide.hackflare.net
```
Names identify nodes. Roles are tracked separately as labels or metadata rather than being encoded into the hostname.  
For example, a node may have:
```
region = eu
role = app
arch = arm64
provider = oracle
```

## Discovery

A Slide fleet has a single discovery name:
```
discovery.slide.hackflare.net
```
The discovery record returns the available Slide nodes. The current plan for this is SVCB records.  
Example:
```dns
discovery.slide.hackflare.net. IN SVCB 1 one.eu.slide.hackflare.net.
discovery.slide.hackflare.net. IN SVCB 1 one.za.slide.hackflare.net.
discovery.slide.hackflare.net. IN SVCB 1 one.us.slide.hackflare.net.
discovery.slide.hackflare.net. IN SVCB 1 two.us.slide.hackflare.net.
```
Using this, a client only needs `discovery.slide.hackflare.net` to find all nodes in the fleet.

It can then:
1. Resolve the fleet.
2. Connect to available nodes.
3. Authenticate to said nodes.
4. Query node capabilities and state.
5. Manage deployments, rolling releases, and other operations.

## Port

`slided` listens on `TCP/1`.  
And don't you worry, you did not read that wrong, this is intentional.  

Slide uses its own protocol initiation system rather than behaving like a normal public HTTP service.
This protocol is initiated using `connect-me-please` as a connection header.

Requests that do not initiate the Slide protocol recieve a deliberately non-useful response and are dropped.

The port number and header name are not security mechanisms. Auth is still handled seperately.

## Authentication

Slide is designed around certficate-based auth.  
The planned trust model is based on TLS:
```
Slide Root CA
    │
    ├── node certificates
    └── client certificates
```
Both sides authenticate each other:
```
client
  │ verifies node certificate
  ▼
slided
  │ verifies client certificate
  ▼
authenticated session
```
A root or intermediate CA can be kept offline while individual nodes and clients recieve their own certificates.  
Though each node must have its own certificate that can create new client certificates. This allows for revocation of individual clients without needing to reissue node certificates.

Because `slided` controls deployments and system networking, access to Slide is treated similarly to root access.

## Deployments

Slide deploys container images to nodes. Pretty obvious.  
A deployment describes things such as:
```
services
image
environment
secrets
ports
health checks
replicas
resource limits
proxy config
```
An example concept:
```
{
  "service": "hackflare-app",
  "image": "ghcr.io/hackflare/hackflare:8f72c91",
  "replicas": 1,
  "healthcheck": "/health"
}
```

Do not take that format as final. JSON is far too enefficient afterall.

## Rolling Release :3

One of Slide's main jobs is zero downtime deployments, aka rolling releases.

Instead of stopping the currently running service first, Slide starts the replacement beside it.
```
current instance
      │ serving traffic
      ▼

pull new image
      │
      ▼
start new instance
      │
      ▼
health check
      │
      ▼
switch proxy
      │
      ▼
drain old instance
      │
      ▼
stop old instance
```
The local slide proxy can route traffic between service generations without requiring the application itself to own the listening port.  
Conceptually:
```
          :80 / :443
              │
              ▼
          Slide proxy
      ┌───────┴───────┐
      ▼               ▼
    old instance    new instance
    :32781          :32782
```
Once the replacement becomes health, Slide atomically switches new traffic to it and drains the old instance.

## Generations

Slide deployments are versioned as generations.

Instead of thinking only in terms of "restart this container", Slide tracks desired and actual deployment state.

Example:
```
Hackflare App - Generation 381

EU-1    healthy      381
ZA-1    healthy      381
US-1    deploying    380 -> 381
```
A generation can include, but is not limited to:
- Container image
- Environment revision
- Secrets revision
- Proxy configuration
- Deployment settings (health checks, resource limits, etc)

This makes rollout state and rollback behavior explicit and trackable.  
Rollbacks then simply request older generations.

## Load balancing

Slide can manage load balancing nodes as part of the same fleet.

A load balancer could receive configuration such as:
```
service: hackflare-web

targets:
  one.za:8080
  one.eu:8080
  one.us:8080

strategy:
  least-connections

health:
  GET /health
  interval: 5s
```
The slide client can update this config, valide it, deploy it to relevant nodes, and report which generation is active.

## Clients

Slide is designed so multiple client interfaces can use the same node API.

Possible clients include:
- CLI
- TUI
- GUI
- Web interface
- API

A client starts with the fleet discovery hostname and trusted client certificate.

The interface should expose operations such as:
- deploy service
- restart service
- rollback deployment
- inspect node
- view logs
- view metrics
- change configuration
- manage secrets
- manage load balancers

## Multi-region operation

Slide is intended to manage geographically distributed infrastructure.  
For example a deployment may resemble something along the lines of:
```
ZA
├── app
├── dns
└── load balancer

EU
├── app
├── dns
├── database
└── load balancer

US
├── app
├── dns
└── load balancer

AU
├── app
└── dns
```
The same discovery and management model applies regardless of provider or CPU architecture.  
Both `amd64` and `arm64` nodes are intended to be supported. With possible support for `amd32` and `arm32` in the future.

## Security model
`slided` is expected to run as root because it may need to:
- manage containers
- bind privileged ports
- manage public listeners
- switch proxy targets
- manage networking
- control service processes
- read protected deployment configuration and secrets
Because of that, Slide should keep its remote API deliberately small.

Current security goals include:
- mutual TLS
- certificate based identities
- no password authentication by default
- strict request validation
- limited API surface area
- no arbitrary remote shell endpoint
- audit logging
- deployment history
- explicit permissions
- safe rollback
- rate limiting

Security through obscurity is not a part of the trust model.

## Hackflare

Slide is being developed under Hackflare's organizaation, primarily for Hackflare's own infrastructure.

The project is inteded to eventually replace Hackflare's dependences on coolify for production deployments.

## License 

TBD.