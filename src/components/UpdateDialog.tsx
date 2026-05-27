import { useEffect, useState } from "react"
import { check, type Update } from "@tauri-apps/plugin-updater"
import { relaunch } from "@tauri-apps/plugin-process"
import {
  AlertDialog,
  AlertDialogAction,
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
    <AlertDialog open={open} onOpenChange={setOpen}>
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
          <AlertDialogAction onClick={handleUpdate} disabled={updating}>
            {updating ? "Atualizando..." : "Atualizar"}
          </AlertDialogAction>
        </AlertDialogFooter>
      </AlertDialogContent>
    </AlertDialog>
  )
}
