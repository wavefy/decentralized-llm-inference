import React, { useState } from "react";
import { ModelStatus, P2PStatus } from "@/lib/p2p";
import { Card, CardContent, CardHeader, CardTitle } from "./ui/card";
import {
  CheckCircledIcon,
  CrossCircledIcon,
  MinusCircledIcon,
  ReloadIcon,
  CopyIcon,
} from "@radix-ui/react-icons";
import { Button } from "./ui/button";
import { stopP2pSession } from "@/api/p2p";
import { controlBasePath, shortenAddress } from "@/lib/utils";
import { useQueryClient } from "@tanstack/react-query";
import {
  Tooltip,
  TooltipContent,
  TooltipProvider,
  TooltipTrigger,
} from "./ui/tooltip";
import { Input } from "./ui/input";
import { deposit } from "@/lib/contract";
import { toast } from "sonner";
import { Alert } from "./ui/alert";

interface P2PStatusWidgetProps {
  status: P2PStatus | undefined;
}

const P2PStatusWidget: React.FC<P2PStatusWidgetProps> = ({ status }) => {
  const [depositAmount, setDepositAmount] = useState<string>("");
  const [isDepositing, setIsDepositing] = useState<boolean>(false);
  const queryClient = useQueryClient();

  if (!status) {
    return null;
  }

  const handleDeposit = async () => {
    if (!depositAmount) {
      toast.error("Please enter a deposit amount");
      return;
    }

    setIsDepositing(true);

    try {
      const amount = parseFloat(depositAmount);
      if (isNaN(amount) || amount <= 0) {
        throw new Error("Invalid deposit amount");
      }

      const txHash = await deposit(amount);
      toast.success(`Deposit successful. Transaction hash: ${txHash}`);
      setDepositAmount("");
      queryClient.invalidateQueries({ queryKey: ["P2P-STATUS"] });
    } catch (err) {
      toast.error(
        err instanceof Error
          ? err.message
          : "An unknown error occurred during deposit"
      );
    } finally {
      setIsDepositing(false);
    }
  };

  return (
    <Card className="mb-4">
      <CardHeader>
        P2p Status
      </CardHeader>
      <CardContent>
        <div className="text-xs space-y-1">
          {status.models.map((model) => <P2pModelStatus status={model} />)}
          {/* Add Deposit functionality */}
          <div className="mt-4 space-y-2">
            <Input
              type="number"
              value={depositAmount}
              onChange={(e) => setDepositAmount(e.target.value)}
              placeholder="Enter deposit amount"
              size={10}
            />
            <Button
              onClick={handleDeposit}
              disabled={isDepositing || !depositAmount}
              className="w-full"
              size="sm"
            >
              {isDepositing ? (
                <>
                  <ReloadIcon className="mr-2 h-4 w-4 animate-spin" />
                  Depositing...
                </>
              ) : (
                "Deposit"
              )}
            </Button>
            <p className="text-xs text-yellow-500">
              Deposit amount should be in 10 to the power of 8, e.g. 100000000 equals 1 APT
            </p>
          </div>
        </div>
      </CardContent>
    </Card>
  );
};

export default P2PStatusWidget;

function P2pModelStatus({ status }: { status: ModelStatus }) {
  const queryClient = useQueryClient();
  const [isStopping, setIsStopping] = React.useState(false);

  const getStatusIcon = (status: string) => {
    switch (status) {
      case "ready":
        return <CheckCircledIcon className="w-4 h-4 text-green-500" />;
      case "incomplete":
        return <MinusCircledIcon className="w-4 h-4 text-yellow-500" />;
      case "stopped":
        return <CrossCircledIcon className="w-4 h-4 text-red-500" />;
      default:
        return null;
    }
  };

  const getStatusColor = (status: string) => {
    switch (status) {
      case "ready":
        return "text-green-500";
      case "incomplete":
        return "text-yellow-500";
      case "stopped":
        return "text-red-500";
      default:
        return "text-gray-500";
    }
  };

  const copyToClipboard = (text: string) => {
    navigator.clipboard.writeText(text).then(() => {
      toast.success("Copied to clipboard");
    });
  };

  const handleStop = async () => {
    setIsStopping(true);
    try {
      await stopP2pSession(controlBasePath);
      queryClient.invalidateQueries({ queryKey: ["P2P-STATUS"] });
    } catch (error) {
      console.error("Failed to stop P2P session:", error);
    } finally {
      setIsStopping(false);
    }
  };

  return (<>
    {status.status !== "stopped" && (
      <Button
        variant="destructive"
        size="sm"
        onClick={handleStop}
        disabled={isStopping}
      >
        {isStopping ? (
          <ReloadIcon className="w-4 h-4 animate-spin mr-2" />
        ) : null}
        Stop
      </Button>
    )}
    <p className="flex items-center gap-2">
      <span>Status:</span>
      <span
        className={`flex items-center gap-1 font-semibold ${getStatusColor(status.status)}`}
      >
        {isStopping ? (
          <ReloadIcon className="w-4 h-4 animate-spin text-yellow-500" />
        ) : (
          getStatusIcon(status.status)
        )}
        {isStopping ? "Stopping..." : status.status}
      </span>
    </p>
    {status.status !== "stopped" && (
      <>
        <p>Model: {status.model}</p>
        <p>
          Layers: {status.from_layer} - {status.to_layer}
        </p>
        <p>Peers: {status.peers}</p>
        <p>Sessions: {status.sessions}</p>
        <p>
          Balance:{" "}
          {status.wallet.balance
            ? (status.wallet.balance! / 100000000).toFixed(5)
            : "..."}
          {" "}APT
        </p>
        <p>
          Topup:{" "}
          {status.wallet.topup_balance
            ? (status.wallet.topup_balance / 100000000).toFixed(5)
            : "..."}
          {" "}APT
        </p>
        <p>Spending: {status.wallet.spending}</p>
        <p>Earning: {status.wallet.earning}</p>
        <p className="flex items-center gap-2">
          <span>Wallet:</span>
          <TooltipProvider>
            <Tooltip>
              <TooltipTrigger asChild>
                <span
                  className="cursor-pointer flex items-center gap-1"
                  onClick={() => copyToClipboard(status.wallet.address!)}
                >
                  {shortenAddress(status.wallet.address)}
                  <CopyIcon className="w-3 h-3" />
                </span>
              </TooltipTrigger>
              <TooltipContent>
                <p>{status.wallet.address}</p>
              </TooltipContent>
            </Tooltip>
          </TooltipProvider>
        </p>
      </>
    )}
  </>)
}
