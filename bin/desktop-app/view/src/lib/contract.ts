import {
  Account,
  Aptos,
  AptosConfig,
  Network,
  Ed25519PrivateKey,
} from "@aptos-labs/ts-sdk";
import { contractAddress } from "./utils";

const config = new AptosConfig({
  network: Network.TESTNET,
});
const client = new Aptos(config);

export async function deposit(amount: number) {
  try {
    const account = await getAccount();

    const txnRequest = await client.transaction.build.simple({
      sender: account.accountAddress,
      data: {
        function: `${contractAddress}::dllm::deposit`,
        functionArguments: [amount],
      },
    });

    const txn = await client.signAndSubmitTransaction({
      transaction: txnRequest,
      signer: account,
    });

    return txn.hash;
  } catch (error) {
    console.error("Error in deposit function:", error);
    throw error;
  }
}

async function getAccount() {
  const privateKeyHex = localStorage.getItem("dllm_pk");
  if (!privateKeyHex) {
    throw new Error("No private key found. Please generate a new account.");
  }
  return Account.fromPrivateKey({ privateKey: new Ed25519PrivateKey(privateKeyHex.replace(/"/g, "")) });
}

export function generateNewAccount(): { address: string; privateKey: string } {
  const account = Account.generate();
  const privateKeyHex = account.privateKey.toString();
  const address = account.accountAddress.toString();
  
  return { address, privateKey: privateKeyHex };
}

export function fundAccount(address: string, amount: number) {
  return client.fundAccount({ accountAddress: address, amount });
}

export function getStoredAccountAddress(): string | null {
  const privateKeyHex = localStorage.getItem("dllm_pk");
  if (!privateKeyHex) {
    return null;
  }
  const account = Account.fromPrivateKey({ privateKey: new Ed25519PrivateKey(privateKeyHex.replace(/"/g, "")) });
  return account.accountAddress.toString();
}

export async function getAccountBalance(address: string): Promise<string> {
  try {
    const resources = await client.getAccountResources({ accountAddress: address });
    const accountResource = resources.find((r) => r.type === "0x1::coin::CoinStore<0x1::aptos_coin::AptosCoin>");
    if (accountResource) {
      const balance = (accountResource.data as { coin: { value: string } }).coin.value;
      return (parseInt(balance) / 100000000).toFixed(8); // Convert from octas to APT
    }
    return "0";
  } catch (error) {
    console.error("Error fetching account balance:", error);
    throw error;
  }
}
