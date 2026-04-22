"use client"

import { Badge } from "@/components/ui/badge"
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "@/components/ui/table"

type Column = { key: string; label: string; className?: string }

type Props = {
  title: string
  count: number
  columns: Column[]
  rows: React.ReactNode[][]
  note?: string
}

export function DataSection({ title, count, columns, rows, note }: Props) {
  return (
    <div className="overflow-hidden rounded-xl border bg-card">
      <div className="flex items-center justify-between border-b bg-muted/30 px-4 py-2">
        <h2 className="text-sm font-semibold tracking-tight">{title}</h2>
        <Badge variant="secondary" className="h-5 font-mono tabular-nums">
          {count}
        </Badge>
      </div>

      <div className="overflow-x-auto">
        <Table>
          <TableHeader>
            <TableRow className="hover:bg-transparent">
              {columns.map((col) => (
                <TableHead
                  key={col.key}
                  className={`h-7 px-3 text-[0.65rem] font-semibold tracking-wider text-muted-foreground uppercase ${col.className ?? ""}`}
                >
                  {col.label}
                </TableHead>
              ))}
            </TableRow>
          </TableHeader>
          <TableBody>
            {rows.length === 0 ? (
              <TableRow>
                <TableCell
                  colSpan={columns.length}
                  className="py-8 text-center text-sm text-muted-foreground"
                >
                  No data
                </TableCell>
              </TableRow>
            ) : (
              rows.map((cells, i) => (
                <TableRow key={i}>
                  {cells.map((cell, j) => (
                    <TableCell
                      key={j}
                      className="px-3 py-1.5 align-top whitespace-normal"
                    >
                      {cell}
                    </TableCell>
                  ))}
                </TableRow>
              ))
            )}
          </TableBody>
        </Table>
      </div>

      {note && (
        <p className="border-t px-4 py-2 text-[0.7rem] text-muted-foreground">
          {note}
        </p>
      )}
    </div>
  )
}
