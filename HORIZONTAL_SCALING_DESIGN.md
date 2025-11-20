# Horizontal Scaling Design

**Status**: Design Document (Not Yet Implemented)
**Priority**: LOW (Implement when single server reaches 70% capacity)
**Created**: November 18, 2025

---

## Overview

This document outlines the design for horizontal scaling of the Private Poker platform to support multiple server instances and handle increased load beyond single-server capacity.

**Current Capacity** (Single Server):
- 500-1,000 concurrent tables
- 5,000-10,000 concurrent players
- 10,000+ requests/sec

**Horizontal Scaling Goal**:
- Unlimited concurrent tables (distributed)
- 50,000+ concurrent players
- 100,000+ requests/sec

---

## Current Architecture (Single Server)

```
┌──────────────┐
│   Clients    │
└──────┬───────┘
       │
┌──────▼───────┐
│  pp_server   │
│  (Single)    │
│              │
│  ┌────────┐  │
│  │ Tables │  │  <- In-memory Actor model
│  │ (Actors)│  │
│  └────────┘  │
└──────┬───────┘
       │
┌──────▼───────┐
│  PostgreSQL  │
└──────────────┘
```

**Limitations**:
- ❌ Table state is in-memory (single server only)
- ❌ WebSocket connections tied to one server
- ❌ No load balancing across servers
- ❌ Single point of failure

---

## Proposed Architecture (Horizontal Scaling)

```
┌──────────────────────────────────────────┐
│              Load Balancer                │
│         (HAProxy / Nginx / ALB)          │
└───────┬──────────────┬───────────────┬──┘
        │              │               │
┌───────▼──────┐  ┌────▼──────┐  ┌────▼──────┐
│  pp_server_1 │  │pp_server_2│  │pp_server_3│
│              │  │           │  │           │
│  ┌────────┐  │  │ ┌────────┐│  │ ┌────────┐│
│  │ Tables │  │  │ │ Tables ││  │ │ Tables ││
│  └───┬────┘  │  │ └───┬────┘│  │ └───┬────┘│
└──────┼───────┘  └─────┼─────┘  └─────┼─────┘
       │                │              │
       └────────┬───────┴──────────────┘
                │
       ┌────────▼─────────┐
       │  Redis Cluster   │ <- Distributed state
       │  (Table Registry │
       │   + PubSub)      │
       └────────┬─────────┘
                │
       ┌────────▼─────────┐
       │    PostgreSQL    │ <- Persistent storage
       │    (Primary)     │
       └──────────────────┘
```

---

## Key Components

### 1. Load Balancer

**Purpose**: Distribute incoming requests across multiple server instances

**Options**:
- **HAProxy**: Open-source, high-performance
- **Nginx**: Also provides reverse proxy
- **AWS ALB**: Cloud-native, auto-scaling
- **Traefik**: Cloud-native, Docker-friendly

**Algorithm**: Least connections (best for WebSocket)

**Health Checks**: HTTP endpoint `/health` on each server

**Configuration Example (HAProxy)**:
```haproxy
frontend poker_frontend
    bind *:8080
    mode http
    default_backend poker_servers

backend poker_servers
    mode http
    balance leastconn
    option httpchk GET /health
    server server1 10.0.1.10:8080 check
    server server2 10.0.1.11:8080 check
    server server3 10.0.1.12:8080 check
```

### 2. Redis Cluster

**Purpose**: Distributed state management and pub/sub messaging

**Use Cases**:
1. **Table Registry**: Track which server owns which table
2. **Session Storage**: Share session data across servers
3. **Pub/Sub**: Broadcast game events
4. **Caching**: Cache frequently accessed data

**Data Structures**:
```
// Table ownership
table:123 -> "server_2"

// Active sessions
session:abc123 -> {user_id, expires_at, ...}

// Table state snapshot (optional cache)
table_state:123 -> {players, pot, board, ...}

// Pub/Sub channels
channel:table:123 -> game events
channel:global -> system events
```

**Implementation**:
```rust
use redis::AsyncCommands;

pub struct RedisTableRegistry {
    client: redis::Client,
}

impl RedisTableRegistry {
    pub async fn register_table(&self, table_id: i64, server_id: &str) -> Result<()> {
        let mut conn = self.client.get_async_connection().await?;
        conn.set(format!("table:{}", table_id), server_id).await?;
        Ok(())
    }

    pub async fn get_table_server(&self, table_id: i64) -> Result<Option<String>> {
        let mut conn = self.client.get_async_connection().await?;
        conn.get(format!("table:{}", table_id)).await
    }

    pub async fn publish_event(&self, table_id: i64, event: GameEvent) -> Result<()> {
        let mut conn = self.client.get_async_connection().await?;
        let channel = format!("channel:table:{}", table_id);
        let payload = serde_json::to_string(&event)?;
        conn.publish(channel, payload).await?;
        Ok(())
    }
}
```

### 3. Session Affinity (Sticky Sessions)

**Purpose**: Keep WebSocket connections on the same server

**Options**:
- **Cookie-based**: Load balancer uses cookie to route to same server
- **IP-based**: Route based on client IP (less reliable)
- **Connection upgrade**: Use HTTP API to determine server, then WebSocket upgrade

**Recommended**: Cookie-based with Redis session storage as fallback

### 4. Table Distribution Strategy

**Strategy 1: Consistent Hashing**
- Assign tables to servers based on hash(table_id)
- Minimizes redistribution when servers added/removed

**Strategy 2: Dynamic Assignment**
- Assign new tables to least-loaded server
- Track table count per server in Redis

**Recommended**: Dynamic assignment for better load balancing

### 5. Message Broadcasting

**For game events that need to reach all players**:

**Current (Single Server)**:
```rust
// Direct broadcast to all connected WebSocket clients
for client in &table.clients {
    client.send(game_view).await;
}
```

**Proposed (Multi-Server)**:
```rust
// Publish to Redis, each server subscribes and forwards to local clients
redis.publish(channel, game_event).await;

// Each server subscribes:
let mut pubsub = redis.subscribe("channel:table:123").await;
while let Some(msg) = pubsub.next().await {
    // Forward to local WebSocket clients connected to this server
    for local_client in &local_clients {
        local_client.send(msg.payload).await;
    }
}
```

---

## Migration Path

### Phase 5.1: Preparation (CURRENT)

- [x] Document scaling architecture
- [x] Identify components needing changes
- [ ] Add Redis dependency to Cargo.toml
- [ ] Create abstraction layer for state management

### Phase 5.2: Redis Integration

- [ ] Add Redis connection pool
- [ ] Implement RedisTableRegistry
- [ ] Implement RedisSessionStore
- [ ] Migrate session storage to Redis

### Phase 5.3: Table Distribution

- [ ] Implement table assignment logic
- [ ] Add server health monitoring
- [ ] Implement table migration on server failure

### Phase 5.4: WebSocket Scaling

- [ ] Implement pub/sub for game events
- [ ] Add sticky session support
- [ ] Test WebSocket failover

### Phase 5.5: Load Balancer Setup

- [ ] Configure HAProxy/Nginx
- [ ] Add health check endpoint
- [ ] Test load distribution

### Phase 5.6: Production Deployment

- [ ] Deploy multiple server instances
- [ ] Configure monitoring and alerts
- [ ] Test failure scenarios
- [ ] Document operational procedures

---

## Code Changes Required

### 1. Add Redis Dependency

**Cargo.toml**:
```toml
[dependencies]
redis = { version = "0.24", features = ["tokio-comp", "connection-manager"] }
```

### 2. Abstract State Management

**Before (In-Memory)**:
```rust
pub struct TableManager {
    tables: HashMap<TableId, TableActor>,
}
```

**After (Redis-Backed)**:
```rust
pub trait StateStore: Send + Sync {
    async fn register_table(&self, table_id: TableId, server_id: ServerId) -> Result<()>;
    async fn get_table_server(&self, table_id: TableId) -> Result<Option<ServerId>>;
    async fn unregister_table(&self, table_id: TableId) -> Result<()>;
}

pub struct TableManager {
    tables: HashMap<TableId, TableActor>, // Local tables only
    state_store: Arc<dyn StateStore>,
}
```

### 3. Server Identification

Add server ID to distinguish instances:

```rust
pub struct ServerConfig {
    pub server_id: String, // e.g., "server-1", "server-2"
    pub redis_url: String,
    pub database_url: String,
}
```

### 4. Health Check Endpoint

```rust
async fn health_check(
    State(app_state): State<AppState>,
) -> Result<Json<HealthStatus>, StatusCode> {
    // Check database
    app_state.db.health_check().await
        .map_err(|_| StatusCode::SERVICE_UNAVAILABLE)?;

    // Check Redis
    app_state.redis.ping().await
        .map_err(|_| StatusCode::SERVICE_UNAVAILABLE)?;

    Ok(Json(HealthStatus {
        status: "healthy",
        server_id: app_state.server_id,
        table_count: app_state.table_manager.table_count(),
        uptime_secs: app_state.uptime(),
    }))
}
```

---

## Performance Considerations

### Redis Performance

**Expected Latency**:
- Same datacenter: 1-2ms
- Cross-region: 50-100ms (avoid if possible)

**Throughput**:
- Single Redis instance: 100k+ ops/sec
- Redis Cluster: 1M+ ops/sec

### Network Bandwidth

**Per Table** (10 players, 1 update/sec):
- View size: ~2KB
- 10 players × 2KB = 20KB/sec per table
- 1,000 tables = 20MB/sec

**With Compression**:
- gzip: ~70% reduction
- 20MB/sec → 6MB/sec (achievable)

### Database Load

**Current**: All queries hit PostgreSQL
**Optimized**: Cache frequently-read data in Redis
- Session validation: Redis (was DB)
- Table configuration: Redis (was DB)
- User balance: Redis with write-through (was DB)

**Expected Reduction**: 50-70% fewer DB queries

---

## Failure Scenarios

### Server Failure

**Detection**: Load balancer health check fails (5s)

**Response**:
1. Load balancer removes server from pool
2. Tables on failed server become unavailable
3. Players see disconnection, can reconnect to lobby
4. New tables assigned to remaining servers

**Recovery**:
- Manual: Restart server, tables recreated
- Automatic: Container orchestration (Kubernetes)

### Redis Failure

**Impact**: Cannot create new tables, session issues

**Mitigation**:
- Redis Sentinel: Automatic failover to replica
- Redis Cluster: Partition tolerance

**Fallback**: Degrade to read-only mode, allow existing games to complete

### Database Failure

**Impact**: Cannot save game results, wallet updates

**Mitigation**:
- PostgreSQL replication (hot standby)
- Automatic failover with pg_auto_failover

**Fallback**: Queue operations in Redis, replay when DB recovers

---

## Monitoring & Observability

### Metrics to Track

**Per Server**:
- Active tables
- Active WebSocket connections
- CPU/Memory usage
- Request latency (p50, p95, p99)

**Global**:
- Total tables across all servers
- Total players
- Redis hit/miss rate
- Database query time

**Tools**:
- Prometheus: Metrics collection
- Grafana: Visualization
- Alertmanager: Alerts

### Logging

**Centralized Logging**:
- Fluentd/Logstash: Log aggregation
- Elasticsearch: Log storage
- Kibana: Log search/visualization

**Request Correlation**:
- Use request ID across all services
- Trace requests across servers

---

## Cost Estimation

### Infrastructure (AWS Example)

| Component | Instance Type | Count | Monthly Cost |
|-----------|--------------|-------|--------------|
| App Servers | t3.medium | 3 | $90 |
| Redis | ElastiCache (cache.t3.medium) | 1 | $50 |
| Load Balancer | ALB | 1 | $20 |
| Database | RDS (db.t3.medium) | 1 | $70 |
| **Total** | - | - | **$230/month** |

**Scaling Up** (10x capacity):
- App Servers: 10 × t3.large = $600
- Redis: cache.r6g.xlarge = $200
- Database: db.r6g.xlarge = $300
- **Total**: $1,120/month for 50k+ players

---

## Testing Strategy

### Load Testing

**Tools**: k6, Artillery, Gatling

**Scenarios**:
1. **Baseline**: Single server, 1,000 tables
2. **Scaled**: 3 servers, 3,000 tables
3. **Failure**: Kill one server, measure recovery

**Metrics**:
- Request success rate
- Latency percentiles
- WebSocket reconnection time

### Chaos Engineering

**Scenarios**:
- Random server failures
- Network partitions
- Redis outage
- Database slowdown

**Tool**: Chaos Mesh, Gremlin

---

## Recommendation

**When to Implement**:
- ⚠️ NOT NOW: Single server handles 5-10k players easily
- ✅ When monitoring shows >70% capacity consistently
- ✅ When preparing for major launch/marketing push
- ✅ When SLA requirements demand high availability

**Estimated Effort**:
- Phase 5.1-5.3: 2-3 weeks (Redis integration, table distribution)
- Phase 5.4-5.5: 1-2 weeks (WebSocket scaling, load balancer)
- Phase 5.6: 1 week (deployment, testing)
- **Total**: 4-6 weeks of development

**Risk Level**: MEDIUM
- Requires significant architectural changes
- Needs thorough testing
- Operational complexity increases

---

## Conclusion

Horizontal scaling is well-designed and feasible but **NOT NEEDED NOW**. The current single-server architecture can handle 5,000-10,000 concurrent players, which is sufficient for initial launch.

**Implement when**:
- Monitoring shows sustained >70% server capacity
- Business growth demands it
- High availability SLA required

**Until then**: Focus on product features, user experience, and business growth.

---

**Status**: Design Complete, Implementation Deferred ✅

**Next Review**: When server utilization exceeds 50% for 1 week

---
