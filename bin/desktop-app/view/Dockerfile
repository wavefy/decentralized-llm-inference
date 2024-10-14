# syntax=docker/dockerfile:1

# Comments are provided throughout this file to help you get started.
# If you need more help, visit the Dockerfile reference guide at
# https://docs.docker.com/go/dockerfile-reference/

# Want to help us make this template better? Share your feedback here: https://forms.gle/ybq9Krt8jtBL3iCk7

ARG NODE_VERSION=20.11.1
ARG PNPM_VERSION=8.6.5

################################################################################
# Use node image for base image for all stages.
FROM node:${NODE_VERSION}-alpine as base

# Set working directory for all build stages.
WORKDIR /usr/src/app

# Install pnpm.
RUN --mount=type=cache,target=/root/.npm \
    npm install -g pnpm@${PNPM_VERSION}

################################################################################
# Create a stage for installing production dependecies.
FROM base as deps

# Download dependencies as a separate step to take advantage of Docker's caching.
# Leverage a cache mount to /root/.local/share/pnpm/store to speed up subsequent builds.
# Leverage bind mounts to package.json and pnpm-lock.yaml to avoid having to copy them
# into this layer.
RUN --mount=type=bind,source=package.json,target=package.json \
    --mount=type=bind,source=pnpm-lock.yaml,target=pnpm-lock.yaml \
    --mount=type=cache,target=/root/.local/share/pnpm/store \
    pnpm install --prod --frozen-lockfile

################################################################################
# Create a stage for building the application.
FROM deps as build

ARG VITE_VLLM_URL
ARG VITE_VLLM_CONTROLS_URL
ARG VITE_NODIT_GQL_API
ARG VITE_VLLM_CONTRACT
ARG VITE_REGISTRY_URL

# Download additional development dependencies before building, as some projects require
# "devDependencies" to be installed to build. If you don't need this, remove this step.
RUN --mount=type=bind,source=package.json,target=package.json \
    --mount=type=bind,source=pnpm-lock.yaml,target=pnpm-lock.yaml \
    --mount=type=cache,target=/root/.local/share/pnpm/store \
    pnpm install --frozen-lockfile

# Copy the rest of the source files into the image.
COPY . .
# Run the build script.
ENV VITE_VLLM_URL=$VITE_VLLM_URL
ENV VITE_VLLM_CONTROLS_URL=$VITE_VLLM_CONTROLS_URL
ENV VITE_NODIT_GQL_API=$VITE_NODIT_GQL_API
ENV VITE_VLLM_CONTRACT=$VITE_VLLM_CONTRACT
ENV VITE_REGISTRY_URL=$VITE_REGISTRY_URL
ENV VITE_MODE=cloud

RUN pnpm run build

################################################################################
# Create a new stage to run the application with minimal runtime dependencies
# where the necessary files are copied from the build stage.
FROM nginx:alpine

WORKDIR /usr/share/nginx/

RUN rm -rf html
RUN mkdir html

WORKDIR /

ENV NODE_ENV production

COPY ./nginx.conf /etc/nginx
COPY --from=build /usr/src/app/dist /usr/share/nginx/html

EXPOSE 80

# Run the application.
ENTRYPOINT ["nginx", "-g", "daemon off;"]
