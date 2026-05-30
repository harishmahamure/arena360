{{- define "arena360-postgresql.name" -}}
{{- default .Chart.Name .Values.nameOverride | trunc 63 | trimSuffix "-" }}
{{- end }}

{{- define "arena360-postgresql.fullname" -}}
{{- if .Values.fullnameOverride }}
{{- .Values.fullnameOverride | trunc 63 | trimSuffix "-" }}
{{- else }}
{{- $name := default .Chart.Name .Values.nameOverride }}
{{- if contains $name .Release.Name }}
{{- .Release.Name | trunc 63 | trimSuffix "-" }}
{{- else }}
{{- printf "%s-%s" .Release.Name $name | trunc 63 | trimSuffix "-" }}
{{- end }}
{{- end }}
{{- end }}

{{- define "arena360-postgresql.labels" -}}
helm.sh/chart: {{ printf "%s-%s" .Chart.Name .Chart.Version | replace "+" "_" | trunc 63 | trimSuffix "-" }}
app.kubernetes.io/name: {{ include "arena360-postgresql.name" . }}
app.kubernetes.io/instance: {{ .Release.Name }}
{{- if .Chart.AppVersion }}
app.kubernetes.io/version: {{ .Chart.AppVersion | quote }}
{{- end }}
app.kubernetes.io/managed-by: {{ .Release.Service }}
{{- end }}

{{- define "arena360-postgresql.selectorLabels" -}}
app.kubernetes.io/name: {{ include "arena360-postgresql.name" . }}
app.kubernetes.io/instance: {{ .Release.Name }}
{{- end }}

{{- define "arena360-postgresql.secretName" -}}
{{- default .Values.postgresql.auth.existingSecret .Values.secret.name }}
{{- end }}

{{- define "arena360-postgresql.postgresServiceName" -}}
{{- printf "%s-db" (include "arena360-postgresql.fullname" .) }}
{{- end }}

{{- define "arena360-postgresql.pgbouncerServiceName" -}}
{{- printf "%s-pgbouncer" (include "arena360-postgresql.fullname" .) }}
{{- end }}
