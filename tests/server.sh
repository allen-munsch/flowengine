#!/bin/bash

# test-flowengine.sh - Complete Flow Engine Test Script with debugging

set -e  # Exit on error

echo "🚀 Flow Engine Test Script"
echo "=========================="
echo ""

# Colors for output
GREEN='\033[0;32m'
BLUE='\033[0;34m'
YELLOW='\033[1;33m'
RED='\033[0;31m'
NC='\033[0m' # No Color

# Check if server is running
check_server() {
    if ! curl -s http://localhost:3000/health > /dev/null; then
        echo -e "${RED}❌ Server is not running!${NC}"
        echo "Start it with: cargo run --bin flowserver"
        exit 1
    fi
}

# Test 1: Health Check
test_health() {
    echo -e "${BLUE}Test 1: Health Check${NC}"
    RESPONSE=$(curl -s http://localhost:3000/health)
    echo "$RESPONSE" | jq
    echo -e "${GREEN}✓ Health check passed${NC}\n"
}

# Test 2: List Node Types
test_nodes() {
    echo -e "${BLUE}Test 2: List Available Node Types${NC}"
    RESPONSE=$(curl -s http://localhost:3000/api/nodes)
    echo "$RESPONSE" | jq
    echo -e "${GREEN}✓ Node types retrieved${NC}\n"
}

# Test 3: Create Workflow
test_create_workflow() {
    echo -e "${BLUE}Test 3: Create Workflow${NC}"
    
    echo -e "${YELLOW}Request body:${NC}"
    cat examples/github_zen.json | jq
    
    echo -e "${YELLOW}Sending request...${NC}"
    RESPONSE=$(curl -s -w "\nHTTP_CODE:%{http_code}" -X POST http://localhost:3000/api/workflows \
      -H "Content-Type: application/json" \
      -d @examples/github_zen.json)
    
    HTTP_CODE=$(echo "$RESPONSE" | grep "HTTP_CODE:" | cut -d: -f2)
    BODY=$(echo "$RESPONSE" | sed '/HTTP_CODE:/d')
    
    echo -e "${YELLOW}HTTP Status: $HTTP_CODE${NC}"
    echo -e "${YELLOW}Response body:${NC}"
    echo "$BODY" | jq
    
    if [ "$HTTP_CODE" != "201" ]; then
        echo -e "${RED}❌ Failed to create workflow${NC}"
        exit 1
    fi
    
    WORKFLOW_ID=$(echo "$BODY" | jq -r '.id')
    echo "Created workflow: $WORKFLOW_ID"
    echo -e "${GREEN}✓ Workflow created${NC}\n"
    echo "$WORKFLOW_ID"
}

# Test 4: List Workflows
test_list_workflows() {
    echo -e "${BLUE}Test 4: List All Workflows${NC}"
    RESPONSE=$(curl -s http://localhost:3000/api/workflows)
    echo "$RESPONSE" | jq
    echo -e "${GREEN}✓ Workflows listed${NC}\n"
}

# Test 5: Get Specific Workflow
test_get_workflow() {
    local WORKFLOW_ID=$1
    echo -e "${BLUE}Test 5: Get Workflow Details${NC}"
    echo "Workflow ID: $WORKFLOW_ID"
    
    RESPONSE=$(curl -s http://localhost:3000/api/workflows/$WORKFLOW_ID)
    echo "$RESPONSE" | jq
    echo -e "${GREEN}✓ Workflow retrieved${NC}\n"
}

# Test 6: Execute Workflow
test_execute_workflow() {
    local WORKFLOW_ID=$1
    echo -e "${BLUE}Test 6: Execute Workflow${NC}"
    echo "Workflow ID: $WORKFLOW_ID"
    
    echo -e "${YELLOW}Request body:${NC}"
    REQUEST_BODY='{
      "inputs": {
        "url": "https://api.github.com/zen"
      }
    }'
    echo "$REQUEST_BODY" | jq
    
    echo -e "${YELLOW}Sending request...${NC}"
    RESPONSE=$(curl -s -w "\nHTTP_CODE:%{http_code}" -X POST http://localhost:3000/api/workflows/$WORKFLOW_ID/execute \
      -H "Content-Type: application/json" \
      -d "$REQUEST_BODY")
    
    HTTP_CODE=$(echo "$RESPONSE" | grep "HTTP_CODE:" | cut -d: -f2)
    BODY=$(echo "$RESPONSE" | sed '/HTTP_CODE:/d')
    
    echo -e "${YELLOW}HTTP Status: $HTTP_CODE${NC}"
    echo -e "${YELLOW}Response body:${NC}"
    echo "$BODY" | jq
    
    if [ "$HTTP_CODE" != "200" ]; then
        echo -e "${RED}❌ Failed to execute workflow${NC}"
        echo -e "${RED}Response: $BODY${NC}"
    else
        echo -e "${GREEN}✓ Workflow executed${NC}\n"
    fi
}

# Test 7: Execute Data Pipeline
test_data_pipeline() {
    echo -e "${BLUE}Test 7: Execute Data Pipeline Workflow${NC}"
    
    echo -e "${YELLOW}Creating pipeline workflow...${NC}"
    echo -e "${YELLOW}Request body:${NC}"
    cat examples/data_pipeline.json | jq
    
    RESPONSE=$(curl -s -w "\nHTTP_CODE:%{http_code}" -X POST http://localhost:3000/api/workflows \
      -H "Content-Type: application/json" \
      -d @examples/data_pipeline.json)
    
    HTTP_CODE=$(echo "$RESPONSE" | grep "HTTP_CODE:" | cut -d: -f2)
    BODY=$(echo "$RESPONSE" | sed '/HTTP_CODE:/d')
    
    echo -e "${YELLOW}HTTP Status: $HTTP_CODE${NC}"
    echo -e "${YELLOW}Response body:${NC}"
    echo "$BODY" | jq
    
    if [ "$HTTP_CODE" != "201" ]; then
        echo -e "${RED}❌ Failed to create pipeline workflow${NC}"
        echo -e "${RED}Response: $BODY${NC}"
        return
    fi
    
    PIPELINE_ID=$(echo "$BODY" | jq -r '.id')
    echo "Created pipeline: $PIPELINE_ID"
    
    # Execute it
    echo -e "${YELLOW}Executing pipeline...${NC}"
    EXEC_REQUEST='{
      "inputs": {
        "url": "https://jsonplaceholder.typicode.com/users/1"
      }
    }'
    echo "$EXEC_REQUEST" | jq
    
    EXEC_RESPONSE=$(curl -s -w "\nHTTP_CODE:%{http_code}" -X POST http://localhost:3000/api/workflows/$PIPELINE_ID/execute \
      -H "Content-Type: application/json" \
      -d "$EXEC_REQUEST")
    
    EXEC_HTTP_CODE=$(echo "$EXEC_RESPONSE" | grep "HTTP_CODE:" | cut -d: -f2)
    EXEC_BODY=$(echo "$EXEC_RESPONSE" | sed '/HTTP_CODE:/d')
    
    echo -e "${YELLOW}HTTP Status: $EXEC_HTTP_CODE${NC}"
    echo -e "${YELLOW}Response body:${NC}"
    echo "$EXEC_BODY" | jq
    
    if [ "$EXEC_HTTP_CODE" != "200" ]; then
        echo -e "${RED}❌ Failed to execute pipeline${NC}"
        echo -e "${RED}Response: $EXEC_BODY${NC}"
    else
        echo -e "${GREEN}✓ Pipeline executed${NC}\n"
    fi
}

# Test 8: Delete Workflow
test_delete_workflow() {
    local WORKFLOW_ID=$1
    echo -e "${BLUE}Test 8: Delete Workflow${NC}"
    echo "Workflow ID: $WORKFLOW_ID"
    
    RESPONSE=$(curl -s -w "\nHTTP_CODE:%{http_code}" -X DELETE http://localhost:3000/api/workflows/$WORKFLOW_ID)
    
    HTTP_CODE=$(echo "$RESPONSE" | grep "HTTP_CODE:" | cut -d: -f2)
    BODY=$(echo "$RESPONSE" | sed '/HTTP_CODE:/d')
    
    echo -e "${YELLOW}HTTP Status: $HTTP_CODE${NC}"
    echo "$BODY" | jq
    
    echo -e "${GREEN}✓ Workflow deleted${NC}\n"
}

function test_docker_node() {
curl -X POST http://localhost:3000/api/workflows \
  -H "Content-Type: application/json" \
  -d @examples/docker_text_processing.json

# Execute
curl -X POST http://localhost:3000/api/workflows/docker-text-001/execute \
  -H "Content-Type: application/json" \
  -d '{
    "inputs": {
      "url": "https://api.quotable.io/random"
    }
  }' | jq
}


# Run all tests
main() {
    echo -e "${YELLOW}Checking if server is running...${NC}"
    check_server
    echo -e "${GREEN}✓ Server is running${NC}\n"
    
    test_health
    test_nodes
    
    WORKFLOW_ID=$(test_create_workflow)
    
    test_list_workflows
    test_get_workflow "$WORKFLOW_ID"
    test_execute_workflow "$WORKFLOW_ID"
    
    test_data_pipeline
    
    test_delete_workflow "$WORKFLOW_ID"
    
    test_docker_node
    echo ""
    echo -e "${GREEN}========================================${NC}"
    echo -e "${GREEN}✓ All tests completed!${NC}"
    echo -e "${GREEN}========================================${NC}"
}

# Check for required tools
if ! command -v jq &> /dev/null; then
    echo -e "${RED}⚠️  jq is not installed. Install it for pretty output:${NC}"
    echo "   sudo apt install jq  # Ubuntu/Debian"
    echo "   brew install jq      # macOS"
    exit 1
fi

main
