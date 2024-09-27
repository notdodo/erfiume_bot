# mypy: disable-error-code="import-untyped"
"""
Generate architecture diagram.
"""

from diagrams import Cluster, Diagram, Edge
from diagrams.aws.compute import Lambda
from diagrams.aws.database import Dynamodb
from diagrams.aws.network import APIGateway
from diagrams.custom import Custom
from diagrams.gcp.devtools import Scheduler
from diagrams.generic.device import Mobile
from diagrams.saas.cdn import Cloudflare
from diagrams.saas.chat import Telegram

graph_attr = {
    "layout": "dot",
    "splines": "curved",
    "bgcolor": "transparent",
    "beautify": "true",
    "center": "true",
}


with Diagram(
    "", show=True, direction="RL", graph_attr=graph_attr, filename="../assets/erfiume"
):
    user = Mobile("user")
    bot = Telegram("erfiume_bot")
    cf = Cloudflare("erfiume.thedodo.xyz")
    api = Custom(
        "allertameteo.regione.emilia-romagna.it",
        icon_path="../assets/er-allertameteo.png",
    )
    with Cluster("AWS"):
        erfiume_bot = Lambda("erfiume_bot")
        erfiume_fetcher = Lambda("erfiume_fetcher")
        stations = Dynamodb("Stazioni")
        api_gw = APIGateway("API Gateway")
        scheduler = Scheduler("every 24h or 15m")

    user >> Edge(label="/command") >> bot
    bot >> Edge(label="trigger webhook") >> cf
    cf << api_gw >> Edge(label="invoke lambda") >> erfiume_bot
    erfiume_bot >> Edge(label="fetch information") >> stations
    erfiume_bot >> Edge(label="send information") >> user
    api << Edge(label="read information") << erfiume_fetcher
    erfiume_fetcher >> Edge(label="update information") >> stations
    scheduler >> Edge(label="start") >> erfiume_fetcher
