#!/bin/bash

fly machines run --schedule="hourly" --entrypoint="/app/sms-sweeper" --volume="data:/data"
