import { Account, Aptos, AptosConfig, Network } from "@aptos-labs/ts-sdk";

const args = process.argv.slice(2);
const account_count = args[0] || 1;
const accounts = [];
(async function () {
  const config = new AptosConfig({ network: Network.TESTNET });
  const aptos = new Aptos(config);
  for (let i = 0; i < account_count; i++) {
    const account = Account.generate();
    const transaction = await aptos.fundAccount({
      accountAddress: account.accountAddress,
      amount: 100_000_000, // 1 Aptos
    });

    accounts.push({
      address: account.accountAddress.toString(),
      privateKey: account.privateKey.toString(),
      publicKey: account.publicKey.toString(),
    });

    console.log("Created and funded account: ", account.accountAddress.toString());
  }

  console.log("Accounts: \n", JSON.stringify(accounts, null, 2));
})();
