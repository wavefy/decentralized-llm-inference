import React, { useState } from "react";
import { ModelStatus } from "@/lib/p2p";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { toast } from "sonner";
import { deposit } from "@/lib/contract";
import { useQueryClient } from "@tanstack/react-query";
import { appMode, controlBasePath, shortenAddress } from "@/lib/utils";
import { stopP2pSession } from "@/api/p2p";
import {
  CheckCircledIcon,
  CrossCircledIcon,
  MinusCircledIcon,
  ReloadIcon,
  CopyIcon,
} from "@radix-ui/react-icons";
import {
  Tooltip,
  TooltipContent,
  TooltipProvider,
  TooltipTrigger,
} from "@/components/ui/tooltip";

interface P2pStatusDashboardProps {
  status: ModelStatus;
}

const P2pStatusDashboard: React.FC<P2pStatusDashboardProps> = ({ status }) => {
  const [depositAmount, setDepositAmount] = useState<string>("");
  const [isDepositing, setIsDepositing] = useState<boolean>(false);
  const [isStopping, setIsStopping] = useState<boolean>(false);
  const queryClient = useQueryClient();

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

  const handleStop = async () => {
    setIsStopping(true);
    try {
      await stopP2pSession(controlBasePath, status.model);
      queryClient.invalidateQueries({ queryKey: ["P2P-STATUS"] });
    } catch (error) {
      console.error("Failed to stop P2P session:", error);
      toast.error("Failed to stop P2P session");
    } finally {
      setIsStopping(false);
    }
  };

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

  return (
    <div className="space-y-4">
      <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-4 gap-4">
        <Card>
          <CardHeader>
            <CardTitle>Status</CardTitle>
          </CardHeader>
          <CardContent>
            <p className={`flex items-center gap-1 font-semibold ${getStatusColor(status.status)}`}>
              {isStopping ? (
                <ReloadIcon className="w-4 h-4 animate-spin text-yellow-500" />
              ) : (
                getStatusIcon(status.status)
              )}
              {isStopping ? "Stopping..." : status.status}
            </p>
          </CardContent>
        </Card>

        <Card>
          <CardHeader>
            <CardTitle>Model</CardTitle>
          </CardHeader>
          <CardContent>
            <p>{status.model}</p>
            <p>Layers: {status.from_layer} - {status.to_layer}</p>
          </CardContent>
        </Card>

        <Card>
          <CardHeader>
            <CardTitle>Network</CardTitle>
          </CardHeader>
          <CardContent>
            <p>Peers ({status.peers.length}): {status.peers.join(", ")}</p>

            <p>Sessions: {status.sessions}</p>
          </CardContent>
        </Card>

        <Card>
          <CardHeader>
            <CardTitle>Wallet</CardTitle>
          </CardHeader>
          <CardContent>
            <p>Balance: {status.wallet.balance ? (status.wallet.balance / 100000000).toFixed(5) : "..."} APT</p>
            <p>Topup: {status.wallet.topup_balance ? (status.wallet.topup_balance / 100000000).toFixed(5) : "..."} APT</p>
            <p>Spending: {status.wallet.spending}</p>
            <p>Earning: {status.wallet.earning}</p>
            <TooltipProvider>
              <Tooltip>
                <TooltipTrigger asChild>
                  <span
                    className="cursor-pointer flex items-center gap-1"
                    onClick={() => copyToClipboard(status.wallet.address)}
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
          </CardContent>
        </Card>
      </div>

      {status.status !== "stopped" && appMode === "local" && (
        <div className="flex gap-4">
          <Button
            variant="destructive"
            onClick={handleStop}
            disabled={isStopping}
          >
            {isStopping ? (
              <ReloadIcon className="w-4 h-4 animate-spin mr-2" />
            ) : null}
            Stop
          </Button>
          <Input
            type="number"
            value={depositAmount}
            onChange={(e) => setDepositAmount(e.target.value)}
            placeholder="Enter deposit amount"
            className="max-w-xs"
          />
          <Button
            onClick={handleDeposit}
            disabled={isDepositing || !depositAmount}
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
        </div>
      )}
      {appMode === "local" && (
        <p className="text-xs text-yellow-500">
          Deposit amount should be in 10 to the power of 8, e.g. 100000000 equals 1 APT
        </p>
      )}
    </div>
  );
};

export default P2pStatusDashboard;
