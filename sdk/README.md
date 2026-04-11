# Agent Vault v2 SDK

Three functions. That's all you need to give any agent safe,
policy-enforced access to funds on Stellar.

## Install

```bash
npm install @stellar/stellar-sdk
# Copy sdk/index.js into your project
```

## Usage

```javascript
const { initVault, openSession, executePayment } = require('./index');

// 1. Initialize vault
await initVault({
  secretKey: 'S...',
  agentAddress: 'G...',
  tokenAddress: 'CDLZFC3SYJYDZT7K67VZ75HPJVIEUVNIXF47ZG2FB2RMQQVU2HHGCYSC',
  whitelist: ['G...'],
  windowLedgers: 100,
  spendingLimit: 1000000,
});

// 2. Open a session
await openSession({
  secretKey: 'S...',
  sessionAgent: 'G...',
  budget: 500000,
  durationLedgers: 500,
});

// 3. Execute payment
await executePayment({
  secretKey: 'S...',
  ownerAddress: 'G...',
  recipient: 'G...',
  amount: 100,
});
```

## Contract ID
CDZHY5PBD3AVBE4ZJ2NFTZE2VTHZQW7UCYHUMEQDGBXCZRPBAAOFOPZW
Network: Stellar Testnet
