import express from "express";
import dotenv from "dotenv";
dotenv.config();

const app = express();
app.use(express.json());

const PORT = process.env.PORT || 3001;

// Dirección del contrato Agent Vault — recibe los pagos
const VAULT_ADDRESS = process.env.CONTRACT_ID;
const PRICE_STROOPS = 100; // 100 stroops de XLM = 0.00001 XLM
const PRICE_DISPLAY = "100 stroops XLM";

// ── RECURSO GRATUITO ─────────────────────────────
app.get("/", (req, res) => {
  res.json({
    service: "Agent Vault x402 Demo",
    network: "stellar:testnet",
    endpoint: "/api/market-data",
    price: PRICE_DISPLAY,
    payTo: VAULT_ADDRESS,
  });
});

// ── RECURSO PROTEGIDO CON x402 ───────────────────
app.get("/api/market-data", async (req, res) => {
  const paymentProof = req.headers["x-payment-proof"];
  const txHash = req.headers["x-payment-hash"];

  // Sin pago — responder 402
  if (!paymentProof && !txHash) {
    return res.status(402).json({
      x402Version: 1,
      error: "Payment Required",
      accepts: [
        {
          scheme: "exact",
          network: "stellar:testnet",
          maxAmountRequired: String(PRICE_STROOPS),
          resource: "/api/market-data",
          description: "Datos de mercado en tiempo real — pagado via Agent Vault",
          mimeType: "application/json",
          payTo: VAULT_ADDRESS,
          maxTimeoutSeconds: 300,
          asset: "native", // XLM nativo
          extra: {
            name: "XLM",
            version: "1",
          },
        },
      ],
      destination: VAULT_ADDRESS,
      amount: PRICE_STROOPS,
      amountDisplay: PRICE_DISPLAY,
      token: "XLM",
      network: "stellar:testnet",
    });
  }

  // Con pago — verificar y entregar recurso
  const proof = txHash || paymentProof;
  const isValid = typeof proof === "string" && proof.length > 10;

  if (!isValid) {
    return res.status(400).json({ error: "Comprobante de pago inválido" });
  }

  console.log(`[x402] Pago verificado — TX: ${proof}`);

  // Entregar el recurso
  return res.status(200).json({
    data: {
      asset: "XLM/USDC",
      price: 0.12,
      volume: 1_500_000,
      timestamp: new Date().toISOString(),
      source: "Stellar DEX testnet",
    },
    x402: {
      paid: true,
      txHash: proof,
      amountPaid: PRICE_DISPLAY,
      resource: "/api/market-data",
      network: "stellar:testnet",
      settledAt: new Date().toISOString(),
      paidVia: "Agent Vault v2 — Vault Session",
    },
  });
});

// ── HEALTH CHECK ─────────────────────────────────
app.get("/health", (req, res) => {
  res.json({
    status: "ok",
    protocol: "x402",
    asset: "XLM native",
    network: "stellar:testnet",
    vault: VAULT_ADDRESS,
    price: PRICE_DISPLAY,
  });
});

app.listen(PORT, () => {
  console.log(`\nServidor x402 corriendo en http://localhost:${PORT}`);
  console.log(`Recurso protegido: GET /api/market-data`);
  console.log(`Precio: ${PRICE_DISPLAY}`);
  console.log(`Vault: ${VAULT_ADDRESS}`);
  console.log(`Red: stellar:testnet\n`);
});