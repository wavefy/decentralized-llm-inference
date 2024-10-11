import { P2PStatus } from "@/lib/p2p";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogHeader,
  DialogTitle,
} from "./ui/dialog";
import { useState, useEffect } from "react";
import { startP2pSession, stopP2pSession, suggestP2pLayers } from "@/api/p2p";
import { controlBasePath, shortenAddress } from "@/lib/utils";
import { Button } from "./ui/button";
import { Input } from "./ui/input";
import { Alert, AlertTitle, AlertDescription } from "./ui/alert";
import { Label } from "./ui/label";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "./ui/select";
import { ReloadIcon, CopyIcon, ExternalLinkIcon } from "@radix-ui/react-icons";
import useLocalStorageState from "use-local-storage-state";
import { toast } from "sonner";
import { generateNewAccount, getStoredAccountAddress, getAccountBalance, fundAccount } from "@/lib/contract";
import {
  Tooltip,
  TooltipContent,
  TooltipProvider,
  TooltipTrigger,
} from "./ui/tooltip";

const MODELS: any = {
  "llama32-1b": {
    layers: 16,
    memory: 3,
  },
  "llama32-3b": {
    layers: 28,
    memory: 8,
  },
  "llama32-vision-11b": {
    layers: 40,
    memory: 25,
  },
  phi3: {
    layers: 32,
    memory: 4,
  },
};

const MAX_MEMORY_OPTIONS = [
  1, 2, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 24, 32, 48, 64,
];

const P2pConfigWidget = ({ status }: { status?: P2PStatus }) => {
  const [selectedModel, setSelectedModel] = useState<string>("");
  const [maxMemory, setMaxMemory] = useState<number>(8);
  const [warning, setWarning] = useState<string | null>(null);
  const [startLayer, setStartLayer] = useState<number>(0);
  const [endLayer, setEndLayer] = useState<number>(18);
  const [privateKey, setPrivateKey] = useLocalStorageState("dllm_pk", {
    defaultValue: "0x3bba41ade33b801bf3e42a080a699e73654eaf1775fb0afc5d65f5449e55d74b",
  });
  const [accountAddress, setAccountAddress] = useState<string | null>(null);
  const [accountBalance, setAccountBalance] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState<boolean>(false);
  const [isGeneratingAccount, setIsGeneratingAccount] = useState(false);

  useEffect(() => {
    const storedAddress = getStoredAccountAddress();
    setAccountAddress(storedAddress);
    if (storedAddress) {
      updateAccountBalance(storedAddress);
    }
  }, [privateKey]);

  const updateAccountBalance = async (address: string) => {
    try {
      const balance = await getAccountBalance(address);
      setAccountBalance(balance);
    } catch (error) {
      console.error("Failed to fetch account balance:", error);
      setAccountBalance(null);
    }
  };

  const handleSuggest = async () => {
    if (!selectedModel) {
      setWarning("Please select a model first.");
      return;
    }

    try {
      const maxLayers = Math.floor(
        (maxMemory / MODELS[selectedModel].memory) *
          MODELS[selectedModel].layers
      );
      const suggestedLayers = await suggestP2pLayers(
        controlBasePath,
        selectedModel,
        maxLayers,
        MODELS[selectedModel].layers
      );
      if (
        suggestedLayers.from_layer !== undefined &&
        suggestedLayers.from_layer != null &&
        suggestedLayers.to_layer !== undefined &&
        suggestedLayers.to_layer != null
      ) {
        setStartLayer(suggestedLayers.from_layer);
        setEndLayer(suggestedLayers.to_layer);
        setWarning(null);
      } else if (suggestedLayers.min_layers !== undefined) {
        const requiredMemory = Math.ceil(
          (MODELS[selectedModel].memory * suggestedLayers.min_layers) /
            MODELS[selectedModel].layers
        );
        setWarning(
          `Need at least ${requiredMemory}GB of memory for ${suggestedLayers.min_layers} layers.`
        );
      } else {
        setWarning("Unable to determine suggested layers.");
      }
    } catch (err) {
      setError(
        err instanceof Error ? err.message : "An unknown error occurred"
      );
    }
  };

  const handleStart = async () => {
    try {
      setError(null);
      setLoading(true);
      await startP2pSession(controlBasePath, {
        model: selectedModel,
        from_layer: startLayer,
        to_layer: endLayer,
        private_key: privateKey,
      });
    } catch (err) {
      setError(
        err instanceof Error ? err.message : "An unknown error occurred"
      );
      setLoading(false);
    }
  };

  const handleStop = async () => {
    try {
      setError(null);
      await stopP2pSession(controlBasePath);
      setPrivateKey("");
    } catch (err) {
      setError(
        err instanceof Error ? err.message : "An unknown error occurred"
      );
    }
  };

  const handleGenerateAccount = async () => {
    setIsGeneratingAccount(true);
    try {
      const { address, privateKey: newPrivateKey } = generateNewAccount();
      await fundAccount(address, 100000000); // Fund new account with 1 APT
      setPrivateKey(newPrivateKey);
      setAccountAddress(address);
      setAccountBalance("0"); // Assume new account has 0 balance
      toast.success("New account generated and private key saved");
      updateAccountBalance(address);
    } catch (error) {
      console.error("Error generating account:", error);
      toast.error("Failed to generate new account");
    } finally {
      setIsGeneratingAccount(false);
    }
  };

  const copyToClipboard = (text: string) => {
    navigator.clipboard.writeText(text).then(() => {
      toast.success('Copied to clipboard');
    });
  };

  const renderFaucetNotice = () => {
    if (!accountAddress || (accountBalance && parseFloat(accountBalance) > 0)) {
      return null;
    }

    return (
      <Alert className="mt-4">
        <AlertTitle>Fund Your Account</AlertTitle>
        <AlertDescription>
          Your account needs funds to start a P2P session. Please use the Aptos faucet to transfer some APT to your account.
          <div className="mt-2">
            <Button variant="outline" size="sm" asChild>
              <a href="https://aptoslabs.com/testnet-faucet" target="_blank" rel="noopener noreferrer" className="flex items-center">
                Go to Aptos Faucet
                <ExternalLinkIcon className="ml-2 h-4 w-4" />
              </a>
            </Button>
          </div>
        </AlertDescription>
      </Alert>
    );
  };

  return (
    <div className="space-y-6">
      {error && (
        <Alert variant="destructive">
          <AlertTitle>Error</AlertTitle>
          <AlertDescription>{error}</AlertDescription>
        </Alert>
      )}
      {status?.model ? (
        <div>
          <div>
            <h2 className="text-lg font-semibold">Current Model</h2>
            <p className="text-sm text-gray-500">
              {status.model.model} (Layers: {status.model.from_layer} -{" "}
              {status.model.to_layer})
            </p>
          </div>
          <div className="mt-4">
            <Button
              onClick={handleStop}
              variant="destructive"
              className="w-full"
            >
              Stop Model
            </Button>
          </div>
        </div>
      ) : (
        <div>
          <div className="space-y-4 mt-4">
            <div className="flex space-x-4 items-end">
              <div className="flex-grow">
                <Select onValueChange={(value) => setSelectedModel(value)}>
                  <SelectTrigger>
                    <SelectValue placeholder="Select a model" />
                  </SelectTrigger>
                  <SelectContent>
                    {Object.keys(MODELS).map((model) => (
                      <SelectItem key={model} value={model}>
                        {model}
                      </SelectItem>
                    ))}
                  </SelectContent>
                </Select>
              </div>
              <div className="w-1/4">
                <Select onValueChange={(value) => setMaxMemory(Number(value))}>
                  <SelectTrigger>
                    <SelectValue placeholder="Max Memory" />
                  </SelectTrigger>
                  <SelectContent>
                    {MAX_MEMORY_OPTIONS.map((mem) => (
                      <SelectItem key={mem} value={mem.toString()}>
                        {mem}GB
                      </SelectItem>
                    ))}
                  </SelectContent>
                </Select>
              </div>
              <Button onClick={handleSuggest} variant="outline">
                Calculate Suggests
              </Button>
            </div>
            {warning && <Alert variant="destructive">{warning}</Alert>}
            {selectedModel && (
              <div className="space-y-4">
                <p>
                  This model has{" "}
                  <span className="font-semibold">
                    {MODELS[selectedModel].layers}
                  </span>{" "}
                  layers and needs{" "}
                  <span className="font-semibold">
                    {MODELS[selectedModel].memory} GB
                  </span>{" "}
                  of memory.
                </p>
                <div className="grid grid-cols-2 gap-4">
                  <div className="space-y-2">
                    <Label htmlFor="startLayer">Start Layer</Label>
                    <Input
                      id="startLayer"
                      type="number"
                      value={startLayer}
                      onChange={(e) => setStartLayer(Number(e.target.value))}
                      min={0}
                      max={MODELS[selectedModel].layers - 1}
                    />
                  </div>
                  <div className="space-y-2">
                    <Label htmlFor="endLayer">End Layer</Label>
                    <Input
                      id="endLayer"
                      type="number"
                      value={endLayer}
                      onChange={(e) => setEndLayer(Number(e.target.value))}
                      min={startLayer + 1}
                      max={MODELS[selectedModel].layers}
                    />
                  </div>
                </div>
                <div className="space-y-2">
                  <Label htmlFor="privateKey">Private Key</Label>
                  <Input
                    id="privateKey"
                    type="text"
                    value={privateKey}
                    onChange={(e) => setPrivateKey(e.target.value)}
                    placeholder="Enter your private key"
                  />
                </div>
                <div className="space-y-2">
                  <Label htmlFor="account">Account</Label>
                  {accountAddress ? (
                    <div className="flex items-center justify-between">
                      <TooltipProvider>
                        <Tooltip>
                          <TooltipTrigger asChild>
                            <span 
                              className="cursor-pointer flex items-center gap-1"
                              onClick={() => copyToClipboard(accountAddress)}
                            >
                              {shortenAddress(accountAddress)}
                              <CopyIcon className="w-3 h-3" />
                            </span>
                          </TooltipTrigger>
                          <TooltipContent>
                            <p>{accountAddress}</p>
                          </TooltipContent>
                        </Tooltip>
                      </TooltipProvider>
                      {accountBalance && (
                        <span className="text-sm">Balance: {accountBalance} APT</span>
                      )}
                    </div>
                  ) : (
                    <p className="text-sm text-gray-500">No account associated with this private key</p>
                  )}
                </div>
                <Button 
                  onClick={handleGenerateAccount} 
                  className="w-full"
                  disabled={isGeneratingAccount}
                >
                  {isGeneratingAccount ? (
                    <>
                      <ReloadIcon className="mr-2 h-4 w-4 animate-spin" />
                      Generating Account...
                    </>
                  ) : (
                    "Generate New Account"
                  )}
                </Button>
                {renderFaucetNotice()}
              </div>
            )}
          </div>
          <div className="mt-4">
            <Button
              onClick={handleStart}
              disabled={!selectedModel || startLayer >= endLayer || loading || !privateKey || !accountBalance || parseFloat(accountBalance) === 0}
              className="w-full"
            >
              {loading ? (
                <>
                  <ReloadIcon className="mr-2 h-4 w-4 animate-spin" />
                  Please wait{" "}
                </>
              ) : (
                "Start Model"
              )}
            </Button>
          </div>
        </div>
      )}
    </div>
  );
};

export const P2pConfig = ({ status }: { status?: P2PStatus }) => {
  return (
    <Dialog open={status && status.status === "stopped"}>
      <DialogContent>
        <DialogHeader>
          <DialogTitle>P2P Config</DialogTitle>
          <DialogDescription>
            Your P2P session has not been started. Please start a session.
          </DialogDescription>
        </DialogHeader>
        <P2pConfigWidget status={status} />
      </DialogContent>
    </Dialog>
  );
};