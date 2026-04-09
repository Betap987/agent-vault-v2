import dotenv from "dotenv";
import pkg from "@stellar/stellar-sdk";
const {
  Contract,
  Keypair,
  Networks,
  TransactionBuilder,
  BASE_FEE,
  nativeToScVal,
  rpc,
  contract: contractUtils,
} = pkg;
dotenv.config();

const CONTRACT_ID   = process.env.CONTRACT_ID;
const OWNER_ADDRESS = process.env.OWNER_ADDRESS;
const AGENT_SECRET  = process.env.AGENT_SECRET;
const RPC_URL       = process.env.RPC_URL;
const SERVER_URL    = "http://localhost:3001";

const agentKeypair = Keypair.fromSecret(AGENT_SECRET);
const rpcServer    = new rpc.Server(RPC_URL);

console.log(`\nAgent Vault v2 — x402 Demo`);
console.log(`Agente:   ${agentKeypair.publicKey()}`);
console.log(`Owner:    ${OWNER_ADDRESS}`);
console.log(`Vault:    ${CONTRACT_ID}`);
console.log(`Servidor: ${SERVER_URL}\n`);

async function waitForTx(hash) {
  console.log(`Esperando confirmación: ${hash}`);
  for (let i = 0; i < 20; i++) {
    await new Promise(r => setTimeout(r, 1500));
    try {
      const result = await rpcServer.getTransaction(hash);
      if (result.status === "SUCCESS") {
        console.log(`TX confirmada en ledger ${result.ledger}`);
        return result;
      }
      if (result.status === "FAILED") {
        throw new Error(`TX falló: ${hash}`);
      }
      console.log(`  Intento ${i + 1}/20 pendiente...`);
    } catch (err) {
      if (err.message.includes("TX falló")) throw err;
    }
  }
  throw new Error("TX no confirmada en tiempo");
}

async function payFromVaultSession(destination, amount) {
  console.log(`\nEjecutando pago desde vault session...`);
  console.log(`Destino: ${destination}`);
  console.log(`Monto:   ${amount} stroops`);

  const vaultContract = new Contract(CONTRACT_ID);
  const operation = vaultContract.call(
    "execute_session_transfer",
    nativeToScVal(OWNER_ADDRESS,  { type: "address" }),
    nativeToScVal(destination,    { type: "address" }),
    nativeToScVal(BigInt(amount), { type: "i128" }),
  );

  const account = await rpcServer.getAccount(agentKeypair.publicKey());
  const tx = new TransactionBuilder(account, {
    fee: BASE_FEE,
    networkPassphrase: Networks.TESTNET,
  })
    .addOperation(operation)
    .setTimeout(30)
    .build();

  const sim = await rpcServer.simulateTransaction(tx);
  if (sim.error) {
    throw new Error(`Simulación falló: ${sim.error}`);
  }

  const prepared = rpc.assembleTransaction(tx, sim).build();
  prepared.sign(agentKeypair);

  const send = await rpcServer.sendTransaction(prepared);
  if (send.status === "ERROR") {
    throw new Error(`Error TX: ${JSON.stringify(send.errorResult)}`);
  }

  await waitForTx(send.hash);
  return send.hash;
}

async function fetchWithVaultPayment(endpoint) {
  const url = `${SERVER_URL}${endpoint}`;
  console.log(`\nAccediendo a: ${url}`);

  const firstResponse = await fetch(url);
  console.log(`Respuesta inicial: HTTP ${firstResponse.status}`);

  if (firstResponse.status !== 402) {
    const data = await firstResponse.json();
    console.log("Respuesta:", data);
    return data;
  }

  const requirement = await firstResponse.json();
  console.log(`\n[402] Pago requerido:`);
  console.log(`  Monto:   ${requirement.amountDisplay}`);
  console.log(`  Destino: ${requirement.destination}`);
  console.log(`  Token:   ${requirement.token}`);

  const destination = requirement.destination ||
    requirement.accepts?.[0]?.payTo;
  const amount = requirement.amount ||
    requirement.accepts?.[0]?.maxAmountRequired;

  if (!destination || !amount) {
    throw new Error("PaymentRequirement incompleto");
  }

  const txHash = await payFromVaultSession(destination, amount);
  console.log(`\nPago ejecutado desde Agent Vault v2`);
  console.log(`TX Hash: ${txHash}`);

  console.log(`\nEnviando solicitud con comprobante...`);
  const paidResponse = await fetch(url, {
    headers: {
      "x-payment-proof": txHash,
      "x-payment-hash":  txHash,
      "content-type":    "application/json",
    },
  });

  if (!paidResponse.ok) {
    const err = await paidResponse.json();
    throw new Error(`Servidor rechazó: ${JSON.stringify(err)}`);
  }

  const resource = await paidResponse.json();

  console.log(`\n✅ RECURSO RECIBIDO EXITOSAMENTE`);
  console.log(`━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━`);
  console.log(`Asset:    ${resource.data.asset}`);
  console.log(`Precio:   $${resource.data.price}`);
  console.log(`Volumen:  ${resource.data.volume.toLocaleString()}`);
  console.log(`━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━`);
  console.log(`TX Hash:  ${resource.x402.txHash}`);
  console.log(`Pagado:   ${resource.x402.amountPaid}`);
  console.log(`Via:      ${resource.x402.paidVia}`);

  return resource;
}

fetchWithVaultPayment("/api/market-data")
  .then(() => {
    console.log(`\n✓ Flujo x402 completado con Agent Vault v2`);
    process.exit(0);
  })
  .catch(err => {
    console.error(`\n✗ Error:`, err.message);
    process.exit(1);
  });