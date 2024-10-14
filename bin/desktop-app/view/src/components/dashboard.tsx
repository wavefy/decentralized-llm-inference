import React from "react";
import { useQuery } from "@apollo/client";
import {
  GET_CREATED_SESSION,
  GET_CLAIMED_REQUESTS,
  ownerJsonFilter,
} from "@/queries/indexer";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "@/components/ui/table";
import { Button } from "@/components/ui/button";
import { useP2PStatus } from "@/queries/p2p";
import { appMode, controlBasePath } from "@/lib/utils";
import CopyableAddress from "./copyable-address";
import P2pStatusDashboard from "./p2p-status-dashboard";
import { P2pConfig } from "./p2p-config";

const ITEMS_PER_PAGE = 10;

interface CreatedSessionEventData {
  owner: string;
  max_tokens: number;
  session_id: number;
  price_per_token: number;
  addresses: string[];
  layers: number[];
  ts: string;
}

interface ClaimedRequestEventData {
  owner: string;
  claimer: string;
  session_id: number;
  token_count: number;
  total_reward: number;
  ts: string;
}

const Dashboard: React.FC = () => {
  const [createdSessionsPage, setCreatedSessionsPage] = React.useState(0);
  const [claimedRequestsPage, setClaimedRequestsPage] = React.useState(0);

  const { status, isLoading: statusLoading } = useP2PStatus({
    baseControlUrl: controlBasePath,
  });

  const { data: createdSessionsData, loading: createdSessionsLoading } =
    useQuery(GET_CREATED_SESSION, {
      variables: {
        limit: ITEMS_PER_PAGE,
        offset: createdSessionsPage * ITEMS_PER_PAGE,
        jsonFilter: ownerJsonFilter(status?.models[0]?.wallet.address!),
      },
      pollInterval: 5000,
      skip: !status?.models[0]?.wallet.address,
    });

  const { data: claimedRequestsData, loading: claimedRequestsLoading } =
    useQuery(GET_CLAIMED_REQUESTS, {
      variables: {
        limit: ITEMS_PER_PAGE,
        offset: claimedRequestsPage * ITEMS_PER_PAGE,
        jsonFilter: ownerJsonFilter(status?.models[0]?.wallet.address!),
      },
      pollInterval: 5000,
      skip: !status?.models[0]?.wallet.address,
    });

  if (statusLoading) {
    return <div>Loading P2P status...</div>;
  }

  return (
    <div className="container mx-auto p-4 space-y-6">
      <h1 className="text-3xl font-bold">Dashboard</h1>

      <div className="flex justify-between items-center">
        <h2 className="text-2xl font-semibold">Active Models</h2>
        {appMode === "local" && <P2pConfig status={status} />}
      </div>

      {status?.models && status.models.length > 0 ? (
        status.models.map((model, index) => (
          <Card key={index} className="mb-6">
            <CardHeader>
              <CardTitle>Model: {model.model}</CardTitle>
            </CardHeader>
            <CardContent>
              <P2pStatusDashboard status={model} />
            </CardContent>
          </Card>
        ))
      ) : (
        <Card>
          <CardContent>
            <p className="text-center py-4">
              No active models.{" "}
              {appMode === "local"
                ? "Start a new model to begin."
                : "Wait for a model to be assigned."}
            </p>
          </CardContent>
        </Card>
      )}

      <Card>
        <CardHeader>
          <CardTitle>Created Sessions</CardTitle>
        </CardHeader>
        <CardContent>
          {createdSessionsLoading ? (
            <p>Loading...</p>
          ) : (
            <>
              <Table>
                <TableHeader>
                  <TableRow>
                    <TableHead>Time</TableHead>
                    <TableHead>Owner</TableHead>
                    <TableHead>Max Tokens</TableHead>
                    <TableHead>Session ID</TableHead>
                    <TableHead>Price per Token</TableHead>
                    <TableHead>Addresses</TableHead>
                    <TableHead>Layers</TableHead>
                  </TableRow>
                </TableHeader>
                <TableBody>
                  {createdSessionsData?.events.map(
                    (event: any, index: number) => {
                      const data: CreatedSessionEventData = event.data;
                      return (
                        <TableRow key={index}>
                          <TableCell>
                            {new Date(+data.ts * 1000).toLocaleString()}
                          </TableCell>
                          <TableCell>
                            <CopyableAddress
                              address={data.owner}
                              isCurrentUser={status?.models.some(
                                (m) => m.wallet.address === data.owner
                              )}
                            />
                          </TableCell>
                          <TableCell>{data.max_tokens}</TableCell>
                          <TableCell>{data.session_id}</TableCell>
                          <TableCell>{data.price_per_token}</TableCell>
                          <TableCell>{data.addresses.join(", ")}</TableCell>
                          <TableCell>{data.layers.join(", ")}</TableCell>
                        </TableRow>
                      );
                    }
                  )}
                </TableBody>
              </Table>
              <div className="flex justify-between mt-4">
                <Button
                  onClick={() =>
                    setCreatedSessionsPage((prev) => Math.max(0, prev - 1))
                  }
                  disabled={createdSessionsPage === 0}
                >
                  Previous
                </Button>
                <Button
                  onClick={() => setCreatedSessionsPage((prev) => prev + 1)}
                  disabled={createdSessionsData?.events.length < ITEMS_PER_PAGE}
                >
                  Next
                </Button>
              </div>
            </>
          )}
        </CardContent>
      </Card>

      <Card>
        <CardHeader>
          <CardTitle>Claimed Requests</CardTitle>
        </CardHeader>
        <CardContent>
          {claimedRequestsLoading ? (
            <p>Loading...</p>
          ) : (
            <>
              <Table>
                <TableHeader>
                  <TableRow>
                    <TableHead>Time</TableHead>
                    <TableHead>Claimer</TableHead>
                    <TableHead>Owner</TableHead>
                    <TableHead>Session ID</TableHead>
                    <TableHead>Token Count</TableHead>
                    <TableHead>Total Reward</TableHead>
                  </TableRow>
                </TableHeader>
                <TableBody>
                  {claimedRequestsData?.events.map(
                    (event: any, index: number) => {
                      const data: ClaimedRequestEventData = event.data;
                      return (
                        <TableRow key={index}>
                          <TableCell>
                            {new Date(+data.ts * 1000).toLocaleString()}
                          </TableCell>
                          <TableCell>
                            <CopyableAddress
                              address={data.claimer}
                              isCurrentUser={status?.models.some(
                                (m) => m.wallet.address === data.claimer
                              )}
                            />
                          </TableCell>
                          <TableCell>
                            <CopyableAddress
                              address={data.owner}
                              isCurrentUser={status?.models.some(
                                (m) => m.wallet.address === data.owner
                              )}
                            />
                          </TableCell>
                          <TableCell>{data.session_id}</TableCell>
                          <TableCell>{data.token_count}</TableCell>
                          <TableCell>{data.total_reward}</TableCell>
                        </TableRow>
                      );
                    }
                  )}
                </TableBody>
              </Table>
              <div className="flex justify-between mt-4">
                <Button
                  onClick={() =>
                    setClaimedRequestsPage((prev) => Math.max(0, prev - 1))
                  }
                  disabled={claimedRequestsPage === 0}
                >
                  Previous
                </Button>
                <Button
                  onClick={() => setClaimedRequestsPage((prev) => prev + 1)}
                  disabled={claimedRequestsData?.events.length < ITEMS_PER_PAGE}
                >
                  Next
                </Button>
              </div>
            </>
          )}
        </CardContent>
      </Card>
    </div>
  );
};

export default Dashboard;
