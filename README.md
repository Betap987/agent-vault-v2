# Agent Vault v2

> The control layer that makes autonomous payments safe.

**"Give your agents the power to pay — without giving them the power to steal."**

Agents can now pay for anything on the internet.
Agent Vault makes sure they don't lose everything while doing it.

---

## The Problem

AI agents can already reason, plan, and act — but they break the moment they need to pay safely.

Every automated payment system has the same critical flaw. To let a bot operate, it needs full access to funds. One compromised key and everything is gone. On blockchain, there is no undo button.

This is the daily reality for trading teams, companies automating payroll, DeFi protocols distributing rewards, and developers building AI agents that pay for external services. They all assume unnecessary risk because no alternative exists.

---

## The Solution

Agent Vault v2 is a programmable control layer for autonomous financial agents built on Stellar. It separates three things that should never go together: **holding funds**, **deciding how to spend them**, and **executing payments**.

The owner deposits funds into a smart contract and defines the rules. The agent operates strictly within those rules. If something goes wrong, the damage is contained by design — not by luck.

---

## How It Works

**1.** The owner opens a vault session with a budget and expiration time.

**2.** The agent performs a paid API request via x402 (HTTP 402).

**3.** The vault enforces policies and executes the payment — or blocks it.

When the session expires, access is revoked automatically on-chain.

---

## Key Innovations

Agent Vault v2 is not an application — it is a primitive that enables a new class of applications.

### Vault Sessions — The OAuth of Financial Execution
Instead of permanent wallet access, the owner opens a **session**: a time-bounded, budget-limited grant for a specific agent. When the session expires or the budget runs out, access is automatically revoked on-chain. No human needed.

### On-Chain Policy Engine
Every payment runs through five automatic checks before a single token moves: agent authorization, vault status, recipient whitelist, sufficient balance, and spending window limit. Results are typed — the contract never fails silently.

### Auto-Pause
After 3 consecutive failed payment attempts, the vault pauses itself automatically and emits an on-chain event. No monitoring required.

### Multi-Owner Architecture
One deployed contract serves unlimited owners with completely isolated vaults. No owner can access another's funds or configuration.

---

## Ecosystem Integrations

Agent Vault v2 is designed to plug directly into the Stellar agent economy.

**DeFindex** — Idle funds generate yield automatically while waiting to be spent. The owner sends funds to the DeFindex XLM vault and withdraws when needed, keeping capital productive at all times.

**x402** — The native payment protocol for AI agents on Stellar. When an agent needs to access a paid API, the server responds HTTP 402, the agent pays from the vault session, and the resource is delivered. Every payment is policy-checked and recorded on-chain.

**Etherfuse** — Payments from the vault arrive as Mexican pesos in real bank accounts via SPEI. The recipient never needs to interact with blockchain.

---

## Validated on Testnet

| What | Transaction |
|---|---|
| DeFindex deposit | `27d8348504cecd313d14...` |
| x402 full flow | `959d3f405feec6a1dd52...` |
| Vault session payment | Confirmed on ledger 1959795 |

Contract on StellarExpert:
https://stellar.expert/explorer/testnet/contract/CDZHY5PBD3AVBE4ZJ2NFTZE2VTHZQW7UCYHUMEQDGBXCZRPBAAOFOPZW

---

## Deployment

| | |
|---|---|
| **Contract ID** | `CDZHY5PBD3AVBE4ZJ2NFTZE2VTHZQW7UCYHUMEQDGBXCZRPBAAOFOPZW` |
| **Network** | Stellar Testnet |
| **DeFindex Vault** | `CCLV4H7WTLJQ7ATLHBBQV2WW3OINF3FOY5XZ7VPHZO7NH3D2ZS4GFSF6` |

---

## Running Locally

### Contract
```bash
git clone https://github.com/Betap987/agent-vault-v2.git
cd agent-vault-v2
stellar contract build
```

### x402 Demo
```bash
# Terminal 1
cd x402-demo
npm install
node server.js

# Terminal 2
node client.js
```

### Environment
Create `.env` in `x402-demo/`:
```env
AGENT_SECRET=YOUR_SECRET_KEY
OWNER_ADDRESS=YOUR_PUBLIC_KEY
CONTRACT_ID=CDZHY5PBD3AVBE4ZJ2NFTZE2VTHZQW7UCYHUMEQDGBXCZRPBAAOFOPZW
RPC_URL=https://soroban-testnet.stellar.org
PORT=3001
```

---

## Project Status

| Component | Status |
|---|---|
| Soroban contract | Deployed and validated on testnet |
| Vault Sessions | Working |
| Policy engine | Working |
| Auto-pause | Working |
| DeFindex integration | Working |
| x402 integration | Working |
| Etherfuse integration | API key requested — in progress |
| Frontend | In progress |

---

## Why This Matters

Without control, autonomous agents cannot safely participate in the economy.

Agent Vault v2 enables a new class of applications:
- AI agents that pay for APIs without holding private keys
- Autonomous research workflows with defined budgets
- Machine-to-machine service marketplaces
- Programmatic payroll systems with on-chain audit trails

This is not just a vault.

It is the missing control layer for the agent economy.

---

## Built For
Stellar Agentic Payments Hackathon 2026

## License
MIT
