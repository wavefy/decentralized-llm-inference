import { P2PStatus } from "@/lib/p2p";
import { useState, useEffect } from "react";
import { startP2pSession, stopP2pSession, suggestP2pLayers } from "@/api/p2p";
import { appMode, controlBasePath, shortenAddress } from "@/lib/utils";
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
import { ReloadIcon, CopyIcon } from "@radix-ui/react-icons";
import useLocalStorageState from "use-local-storage-state";
import { toast } from "sonner";
import {
  generateNewAccount,
  getStoredAccountAddress,
  getAccountBalance,
  fundAccount,
} from "@/lib/contract";
import {
  Tooltip,
  TooltipContent,
  TooltipProvider,
  TooltipTrigger,
} from "./ui/tooltip";
import { Separator } from "./ui/separator";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogHeader,
  DialogTitle,
  DialogTrigger,
} from "./ui/dialog";

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

interface P2pConfigProps {
  status?: P2PStatus;
}

const P2pConfigWidget = ({ status }: P2pConfigProps) => {
  const [selectedModel, setSelectedModel] = useState<string>("");
  const [maxMemory, setMaxMemory] = useState<number>(8);
  const [warning, setWarning] = useState<string | null>(null);
  const [startLayer, setStartLayer] = useState<number>(0);
  const [endLayer, setEndLayer] = useState<number>(18);
  const [privateKey, setPrivateKey] = useLocalStorageState<string>("dllm_pk");
  const [accountAddress, setAccountAddress] = useState<string | null>(null);
  const [accountBalance, setAccountBalance] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState<boolean>(false);
  const [isGeneratingAccount, setIsGeneratingAccount] = useState(false);
  const [isDialogOpen, setIsDialogOpen] = useState(false);
  const [prevModelCount, setPrevModelCount] = useState(0);

  useEffect(() => {
    const storedAddress = getStoredAccountAddress();
    setAccountAddress(storedAddress);
    if (storedAddress) {
      updateAccountBalance(storedAddress);
    }
  }, [privateKey]);

  useEffect(() => {
    if (status && status.models) {
      const currentModelCount = status.models.length;
      if (currentModelCount > prevModelCount) {
        // A new model has been added
        setIsDialogOpen(false);
      }
      setPrevModelCount(currentModelCount);
    }
  }, [status, prevModelCount]);

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
      if (!accountAddress || !privateKey) {
        throw new Error("No account address found.");
      }
      await startP2pSession(controlBasePath, {
        model: selectedModel,
        from_layer: startLayer,
        to_layer: endLayer,
        private_key: privateKey,
      });
      // Don't close the dialog here, it will be closed by the effect when status updates
    } catch (err) {
      setError(
        err instanceof Error ? err.message : "An unknown error occurred"
      );
      setLoading(false);
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
      toast.success("Copied to clipboard");
    });
  };

  return (
    <Dialog open={isDialogOpen} onOpenChange={setIsDialogOpen}>
      <DialogTrigger asChild>
        <Button>Start New Model</Button>
      </DialogTrigger>
      <DialogContent>
        <DialogHeader>
          <DialogTitle>Start New Model</DialogTitle>
          <DialogDescription>
            Configure and start a new model for P2P processing.
          </DialogDescription>
        </DialogHeader>
        <div className="space-y-6 mt-4">
          {error && (
            <Alert variant="destructive">
              <AlertTitle>Error</AlertTitle>
              <AlertDescription>{error}</AlertDescription>
            </Alert>
          )}
          {appMode === "local" && (
            <>
              <div className="space-y-2 text-center">
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
                <p className="text-xs text-left text-yellow-500">
                  Generating a new Aptos Account and fund it with 1 APT from testnet
                  faucet. Remember to save the generated privatekey somewhere available
                  to you just in case.
                </p>
                <Separator className="my-6" />
                <Label htmlFor="privateKey" className="text-center">
                  Or
                </Label>
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
                  <p className="text-sm text-gray-500">
                    No account associated with this private key
                  </p>
                )}
              </div>
            </>
          )}
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
                        min={startLayer}
                        max={MODELS[selectedModel].layers}
                      />
                    </div>
                  </div>
                </div>
              )}
            </div>
            <div className="mt-4">
              <Button
                onClick={handleStart}
                disabled={
                  !selectedModel ||
                  startLayer > endLayer ||
                  loading ||
                  !privateKey ||
                  !accountBalance ||
                  parseFloat(accountBalance) === 0
                }
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
        </div>
      </DialogContent>
    </Dialog>
  );
};

export const P2pConfig = ({ status }: P2pConfigProps) => {
  return <P2pConfigWidget status={status} />;
};
