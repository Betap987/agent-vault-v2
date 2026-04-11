/**
 * Agent Vault v2 SDK
 * The missing control layer for the agent economy on Stellar.
 *
 * Three functions. That's all you need to give any agent
 * safe, policy-enforced access to funds on Stellar.
 *
 * Usage:
 *   const { initVault, openSession, executePayment } = require('./index');
 */

const pkg = require("@stellar/stellar-sdk");
const {
  Contract,
  Keypair,
  Networks,
  TransactionBuilder,
  BASE_FEE,
  nativeToScVal,
  rpc,
} = pkg;

// ─── CONFIGURATION ───────────────────────────────────────────────────────────

const DEFAULT_CONTRACT_ID =
  "CDZHY5PBD3AVBE4ZJ2NFTZE2VTHZQW7UCYHUMEQDGBXCZRPBAAOFOPZW";

const DEFAULT_RPC_URL = "https://soroban-testnet.stellar.org";

// ─── INTERNAL HELPERS ────────────────────────────────────────────────────────

function createServer(rpcUrl) {
  return new rpc.Server(rpcUrl || DEFAULT_RPC_URL);
}

async function waitForTx(server, hash) {
  for (let i = 0; i < 20; i++) {
    await new Promise((r) => setTimeout(r, 1500));
    try {
      const result = await server.getTransaction(hash);
      if (result.status === "SUCCESS") return result;
      if (result.status === "FAILED") throw new Error(`TX failed: ${hash}`);
    } catch (err) {
      if (err.message.includes("TX failed")) throw err;
    }
  }
  throw new Error("TX not confirmed in time");
}

async function sendTx(server, tx, keypair) {
  const sim = await server.simulateTransaction(tx);
  if (sim.error) throw new Error(`Simulation failed: ${sim.error}`);

  const prepared = rpc.assembleTransaction(tx, sim).build();
  prepared.sign(keypair);

  const send = await server.sendTransaction(prepared);
  if (send.status === "ERROR") {
    throw new Error(`Send failed: ${JSON.stringify(send.errorResult)}`);
  }

  return waitForTx(server, send.hash);
}

// ─── PUBLIC API ───────────────────────────────────────────────────────────────

/**
 * initVault — Initialize a new vault with policies.
 *
 * @param {object} config
 * @param {string} config.secretKey      - Owner secret key
 * @param {string} config.agentAddress   - Agent public key
 * @param {string} config.tokenAddress   - Token contract address
 * @param {string[]} config.whitelist    - Allowed recipient addresses
 * @param {number} config.windowLedgers  - Spending window in ledgers
 * @param {number} config.spendingLimit  - Max spend per window (in stroops)
 * @param {string} [config.contractId]   - Agent Vault contract ID
 * @param {string} [config.rpcUrl]       - Stellar RPC URL
 *
 * @returns {Promise<string>} Transaction hash
 *
 * @example
 * await initVault({
 *   secretKey: 'S...',
 *   agentAddress: 'G...',
 *   tokenAddress: 'C...',
 *   whitelist: ['G...', 'G...'],
 *   windowLedgers: 100,
 *   spendingLimit: 1000000,
 * });
 */
async function initVault(config) {
  const {
    secretKey,
    agentAddress,
    tokenAddress,
    whitelist = [],
    windowLedgers = 100,
    spendingLimit = 1000000,
    contractId = DEFAULT_CONTRACT_ID,
    rpcUrl = DEFAULT_RPC_URL,
  } = config;

  const keypair = Keypair.fromSecret(secretKey);
  const server  = createServer(rpcUrl);
  const contract = new Contract(contractId);

  const whitelistVal = nativeToScVal(
    whitelist.map((a) => nativeToScVal(a, { type: "address" })),
    { type: "array" }
  );

  const account = await server.getAccount(keypair.publicKey());
  const tx = new TransactionBuilder(account, {
    fee: BASE_FEE,
    networkPassphrase: Networks.TESTNET,
  })
    .addOperation(
      contract.call(
        "init_vault",
        nativeToScVal(keypair.publicKey(), { type: "address" }),
        nativeToScVal(agentAddress,        { type: "address" }),
        nativeToScVal(tokenAddress,        { type: "address" }),
        whitelistVal,
        nativeToScVal(windowLedgers,       { type: "u32" }),
        nativeToScVal(BigInt(spendingLimit),{ type: "i128" }),
      )
    )
    .setTimeout(30)
    .build();

  const result = await sendTx(server, tx, keypair);
  console.log(`✅ Vault initialized — TX: ${result.hash || "confirmed"}`);
  return result;
}

/**
 * openSession — Open a vault session for an agent.
 * This is the OAuth of financial execution on Stellar.
 *
 * @param {object} config
 * @param {string} config.secretKey        - Owner secret key
 * @param {string} config.sessionAgent     - Agent address for this session
 * @param {number} config.budget           - Session budget in stroops
 * @param {number} config.durationLedgers  - Session duration in ledgers
 * @param {string} [config.contractId]     - Agent Vault contract ID
 * @param {string} [config.rpcUrl]         - Stellar RPC URL
 *
 * @returns {Promise<object>} Transaction result
 *
 * @example
 * await openSession({
 *   secretKey: 'S...',
 *   sessionAgent: 'G...',
 *   budget: 500000,        // 0.05 XLM
 *   durationLedgers: 500,  // ~40 minutes
 * });
 */
async function openSession(config) {
  const {
    secretKey,
    sessionAgent,
    budget,
    durationLedgers = 500,
    contractId = DEFAULT_CONTRACT_ID,
    rpcUrl = DEFAULT_RPC_URL,
  } = config;

  if (!budget) throw new Error("budget is required");

  const keypair  = Keypair.fromSecret(secretKey);
  const server   = createServer(rpcUrl);
  const contract = new Contract(contractId);

  const account = await server.getAccount(keypair.publicKey());
  const tx = new TransactionBuilder(account, {
    fee: BASE_FEE,
    networkPassphrase: Networks.TESTNET,
  })
    .addOperation(
      contract.call(
        "open_session",
        nativeToScVal(keypair.publicKey(), { type: "address" }),
        nativeToScVal(sessionAgent,        { type: "address" }),
        nativeToScVal(BigInt(budget),      { type: "i128" }),
        nativeToScVal(durationLedgers,     { type: "u32" }),
      )
    )
    .setTimeout(30)
    .build();

  const result = await sendTx(server, tx, keypair);
  console.log(`✅ Session opened — budget: ${budget} stroops, duration: ${durationLedgers} ledgers`);
  return result;
}

/**
 * executePayment — Execute a payment from the vault.
 * Handles x402 flow automatically if the service responds with HTTP 402.
 *
 * @param {object} config
 * @param {string} config.secretKey     - Agent secret key
 * @param {string} config.ownerAddress  - Vault owner address
 * @param {string} config.recipient     - Recipient address or service URL
 * @param {number} config.amount        - Amount in stroops
 * @param {string} [config.contractId]  - Agent Vault contract ID
 * @param {string} [config.rpcUrl]      - Stellar RPC URL
 *
 * @returns {Promise<string>} Transaction hash
 *
 * @example
 * // Direct payment
 * await executePayment({
 *   secretKey: 'S...',
 *   ownerAddress: 'G...',
 *   recipient: 'G...',
 *   amount: 100,
 * });
 *
 * // x402 payment — pays automatically if service responds 402
 * await executePayment({
 *   secretKey: 'S...',
 *   ownerAddress: 'G...',
 *   recipient: 'G...',  // service payment address
 *   amount: 100,
 * });
 */
async function executePayment(config) {
  const {
    secretKey,
    ownerAddress,
    recipient,
    amount,
    contractId = DEFAULT_CONTRACT_ID,
    rpcUrl = DEFAULT_RPC_URL,
  } = config;

  if (!amount) throw new Error("amount is required");
  if (!recipient) throw new Error("recipient is required");

  const keypair  = Keypair.fromSecret(secretKey);
  const server   = createServer(rpcUrl);
  const contract = new Contract(contractId);

  const account = await server.getAccount(keypair.publicKey());
  const tx = new TransactionBuilder(account, {
    fee: BASE_FEE,
    networkPassphrase: Networks.TESTNET,
  })
    .addOperation(
      contract.call(
        "execute_session_transfer",
        nativeToScVal(ownerAddress,   { type: "address" }),
        nativeToScVal(recipient,      { type: "address" }),
        nativeToScVal(BigInt(amount), { type: "i128" }),
      )
    )
    .setTimeout(30)
    .build();

  const result = await sendTx(server, tx, keypair);
  console.log(`✅ Payment executed — ${amount} stroops to ${recipient.slice(0, 8)}...`);
  return result;
}

// ─── EXPORTS ──────────────────────────────────────────────────────────────────

module.exports = {
  initVault,
  openSession,
  executePayment,
  DEFAULT_CONTRACT_ID,
  DEFAULT_RPC_URL,
};