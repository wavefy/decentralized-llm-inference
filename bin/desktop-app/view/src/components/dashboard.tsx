import React, { useState } from "react";
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
import { controlBasePath } from "@/lib/utils";
import CopyableAddress from "./copyable-address";

const ITEMS_PER_PAGE = 10;

interface CreatedSessionEventData {
  owner: string;
  max_tokens: number;
  session_id: number;
  price_per_token: number;
  addresses: string[];
  layers: number[];
}

interface ClaimedRequestEventData {
  owner: string;
  claimer: string;
  session_id: number;
  token_count: number;
  total_reward: number;
}

const Dashboard: React.FC = () => {
  const [createdSessionsPage, setCreatedSessionsPage] = useState(0);
  const [claimedRequestsPage, setClaimedRequestsPage] = useState(0);

  const { status, isLoading: statusLoading } = useP2PStatus({
    baseControlUrl: controlBasePath,
  });

  const { data: createdSessionsData, loading: createdSessionsLoading } =
    useQuery(GET_CREATED_SESSION, {
      variables: {
        limit: ITEMS_PER_PAGE,
        offset: createdSessionsPage * ITEMS_PER_PAGE,
        jsonFilter: ownerJsonFilter(status?.address!),
      },
      pollInterval: 5000,
      skip: !status?.address,
    });

  const { data: claimedRequestsData, loading: claimedRequestsLoading } =
    useQuery(GET_CLAIMED_REQUESTS, {
      variables: {
        limit: ITEMS_PER_PAGE,
        offset: claimedRequestsPage * ITEMS_PER_PAGE,
        jsonFilter: ownerJsonFilter(status?.address!),
      },
      pollInterval: 5000,
      skip: !status?.address,
    });

  if (statusLoading) {
    return <div>Loading P2P status...</div>;
  }

  if (!status?.address) {
    return <div>P2P status not ready. Please start a P2P session.</div>;
  }

  return (
    <div className="container mx-auto p-4 space-y-6">
      <h1 className="text-3xl font-bold">Dashboard</h1>

      <div className="grid grid-cols-1 md:grid-cols-3 gap-4">
        <Card>
          <CardHeader>
            <CardTitle>Sessions</CardTitle>
          </CardHeader>
          <CardContent>
            <p className="text-4xl font-bold">{status.sessions}</p>
          </CardContent>
        </Card>
        <Card>
          <CardHeader>
            <CardTitle>Peers</CardTitle>
          </CardHeader>
          <CardContent>
            <p className="text-4xl font-bold">{status.peers}</p>
          </CardContent>
        </Card>
        <Card>
          <CardHeader>
            <CardTitle>Balance</CardTitle>
          </CardHeader>
          <CardContent>
            <p className="text-4xl font-bold">{status.balance?.toFixed(2)}</p>
          </CardContent>
        </Card>
      </div>

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
                            <CopyableAddress
                              address={data.owner}
                              isCurrentUser={data.owner === status.address}
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
                            <CopyableAddress
                              address={data.claimer}
                              isCurrentUser={data.claimer === status.address}
                            />
                          </TableCell>
                          <TableCell>
                            <CopyableAddress
                              address={data.owner}
                              isCurrentUser={data.owner === status.address}
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
