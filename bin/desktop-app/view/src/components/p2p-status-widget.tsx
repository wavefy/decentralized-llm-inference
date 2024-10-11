import React, { useState } from "react";
import { P2PStatus } from "@/lib/p2p";
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
  const [isStopping, setIsStopping] = React.useState(false);
  const [depositAmount, setDepositAmount] = useState<string>("");
  const [isDepositing, setIsDepositing] = useState<boolean>(false);
  const queryClient = useQueryClient();

  if (!status) {
    return null;
  }

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

  const copyToClipboard = (text: string) => {
    navigator.clipboard.writeText(text).then(() => {
      toast.success("Copied to clipboard");
    });
  };

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
        <CardTitle className="text-sm flex justify-between items-center">
          P2P Status
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
        </CardTitle>
      </CardHeader>
      <CardContent>
        <div className="text-xs space-y-1">
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
              <p>Model: {status.model?.model}</p>
              <p>
                Layers: {status.model?.from_layer} - {status.model?.to_layer}
              </p>
              <p>Peers: {status.peers}</p>
              <p>Sessions: {status.sessions}</p>
              <p>
                Balance:{" "}
                {status.balance
                  ? (status.balance! / 100000000).toFixed(5)
                  : "..."}
                {" "}APT
              </p>
              <p>
                Topup:{" "}
                {status.topup_balance
                  ? (status.topup_balance / 100000000).toFixed(5)
                  : "..."}
                {" "}APT
              </p>
              <p>Spending: {status.spending}</p>
              <p>Earning: {status.earning}</p>
              {status.address && (
                <p className="flex items-center gap-2">
                  <span>Wallet:</span>
                  <TooltipProvider>
                    <Tooltip>
                      <TooltipTrigger asChild>
                        <span
                          className="cursor-pointer flex items-center gap-1"
                          onClick={() => copyToClipboard(status.address!)}
                        >
                          {shortenAddress(status.address)}
                          <CopyIcon className="w-3 h-3" />
                        </span>
                      </TooltipTrigger>
                      <TooltipContent>
                        <p>{status.address}</p>
                      </TooltipContent>
                    </Tooltip>
                  </TooltipProvider>
                </p>
              )}
            </>
          )}

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
