import { useEffect, useState } from "react"
import { check, type Update } from "@tauri-apps/plugin-updater"
import { relaunch } from "@tauri-apps/plugin-process"
import {
  AlertDialog,
  AlertDialogCancel,
  AlertDialogContent,
  AlertDialogDescription,
  AlertDialogFooter,
  AlertDialogHeader,
  AlertDialogTitle,
} from "./ui/alert-dialog"

export function UpdateDialog() {
  const [open, setOpen] = useState(false)
  const [version, setVersion] = useState("")
  const [updating, setUpdating] = useState(false)
  const [error, setError] = useState("")
  const [update, setUpdate] = useState<Update | null>(null)

  useEffect(() => {
    check()
      .then((u) => {
        if (u?.available) {
          setVersion(u.version)
          setUpdate(u)
          setOpen(true)
        }
      })
      .catch((err) => {
        console.error("[updater] falha ao verificar atualização:", err)
      })
  }, [])

  async function handleUpdate() {
    if (!update) return
    setUpdating(true)
    setError("")
    try {
      await update.downloadAndInstall()
      await relaunch()
    } catch (err) {
      console.error("[updater] falha ao instalar:", err)
      setError(String(err))
      setUpdating(false)
    }
  }

  return (
    <AlertDialog open={open} onOpenChange={(v) => { if (!updating) setOpen(v) }}>
      <AlertDialogContent size="sm">
        <AlertDialogHeader>
          <AlertDialogTitle>Atualização disponível</AlertDialogTitle>
          <AlertDialogDescription>
            A versão <strong>{version}</strong> está disponível. Deseja atualizar agora?
          </AlertDialogDescription>
        </AlertDialogHeader>
        {error && (
          <p className="px-1 pb-2 text-xs text-red-500 break-all">{error}</p>
        )}
        <AlertDialogFooter>
          <AlertDialogCancel disabled={updating}>Agora não</AlertDialogCancel>
          <button
            onClick={handleUpdate}
            disabled={updating}
            className="inline-flex items-center justify-center rounded-md bg-primary px-4 py-2 text-sm font-medium text-primary-foreground shadow transition-colors hover:bg-primary/90 disabled:pointer-events-none disabled:opacity-50"
          >
            {updating ? "Atualizando..." : "Atualizar"}
          </button>
        </AlertDialogFooter>
      </AlertDialogContent>
    </AlertDialog>
  )
}
