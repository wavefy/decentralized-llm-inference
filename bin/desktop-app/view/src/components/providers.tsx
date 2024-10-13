"use client";

import { noditApiUrl } from "@/lib/utils";
import { ApolloClient, ApolloProvider, InMemoryCache } from "@apollo/client";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { ThemeProvider } from "next-themes";

const queryClient = new QueryClient();
const client = new ApolloClient({
  uri: noditApiUrl,
  cache: new InMemoryCache(),
});

export const Providers = ({ children }: { children: React.ReactNode }) => (
  <ThemeProvider attribute="class" defaultTheme="dark">
    <ApolloProvider client={client}>
      <QueryClientProvider client={queryClient}>{children}</QueryClientProvider>
    </ApolloProvider>
  </ThemeProvider>
);
