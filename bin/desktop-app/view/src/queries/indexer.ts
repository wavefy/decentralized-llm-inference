import { contractAddress } from "@/lib/utils";
import { gql } from "@apollo/client";

export const GET_CREATED_SESSION = gql`
  query CreatedSessions($jsonFilter: jsonb, $limit: Int, $offset: Int) {
      events(
      where: {indexed_type: {_eq: "${contractAddress}::dllm::SessionCreated"}, data: {_contains: $jsonFilter}}
      offset: $offset
      limit: $limit
      order_by: {transaction_version: desc}
    ) {
      data
      type
    }
  }
`;

export const GET_CLAIMER_REQUESTS = gql`
  query ClaimedRequests($jsonFilter: jsonb, $limit: Int, $offset: Int) {
      events(
      where: {indexed_type: {_eq: "${contractAddress}::dllm::TokenClaimed"}, data: {_contains: $jsonFilter, }}
      offset: $offset
      limit: $limit
      order_by: {transaction_version: desc}
    ) {
      data
      type
    }
  }
`;

export const GET_CLAIMED_REQUESTS = gql`
  query ClaimedRequests($jsonFilter: jsonb, $limit: Int, $offset: Int) {
      events(
      where: {indexed_type: {_eq: "${contractAddress}::dllm::TokenClaimed"}, data: {_contains: $jsonFilter, }}
      offset: $offset
      limit: $limit
      order_by: {transaction_version: desc}
    ) {
      data
      type
    }
  }
`;

export function claimerJsonFilter(claimer: string) {
  return {
    claimer,
  };
}

export function ownerJsonFilter(owner: string) {
  return {
    owner,
  };
}
